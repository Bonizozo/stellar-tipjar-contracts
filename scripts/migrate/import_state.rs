//! # State Import — `StateMigrator::import_state()`
//!
//! Imports a transformed [`StateSnapshot`] into the **target** TipJar contract
//! atomically.  Before any write the current target state is backed up so that
//! [`rollback`] can restore it if anything goes wrong.
//!
//! ## Atomicity guarantee
//! Soroban does not expose multi-key atomic transactions from off-chain tooling,
//! so we achieve logical atomicity through the following protocol:
//!
//! 1. **Backup** — export the target contract's current state to a backup file.
//! 2. **Pause** — pause the target contract so no user transactions can race.
//! 3. **Import** — write every record in the snapshot to the target contract.
//!    If any write fails the loop aborts immediately.
//! 4. **Verify** — run [`verify_migration`] to confirm every record landed.
//! 5. **Unpause** — resume normal operations.
//!
//! If step 3 or 4 fails, [`rollback`] is called automatically, which re-imports
//! the backup snapshot and unpauses the contract.
//!
//! ## Dry-run mode
//! When `config.migration.dry_run` is `true` steps 2–5 are skipped entirely.
//! The snapshot is validated and progress is reported but nothing is written.
//!
//! ## Usage
//! ```bash
//! cargo run --manifest-path scripts/migrate/Cargo.toml \
//!     --bin import -- --config scripts/migrate/config.toml
//! ```

use std::fs;
use std::path::Path;

use crate::export_state::{StateMigrator, verify_checksum};
use crate::history::append_history_entry;
use crate::types::{
    MigrationConfig, MigrationHistoryEntry, MigrationStatus, StateSnapshot,
};
use crate::verify_migration::VerificationEngine;

// ─────────────────────────────────────────────────────────────────────────────
// Import implementation on StateMigrator
// ─────────────────────────────────────────────────────────────────────────────

impl StateMigrator {
    /// Imports `snapshot` into the target contract.
    ///
    /// Returns the number of records successfully imported.
    ///
    /// # Errors
    /// Returns an error string if the import fails.  The contract is left in a
    /// paused state; call [`rollback`] to restore the backup.
    pub fn import_state(&self, snapshot: &StateSnapshot) -> Result<usize, String> {
        let cfg = &self.config.migration;
        let target = &cfg.target_contract_id;
        let started_at = iso_now();

        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║           TipJar State Import — v{:<26}║", snapshot.schema_version);
        println!("╚══════════════════════════════════════════════════════════╝");
        println!("  Target   : {}", target);
        println!("  Network  : {}", cfg.network);
        println!("  Dry-run  : {}", cfg.dry_run);
        println!("  Records  : {}", snapshot.total_records());
        println!();

        // ── 0. Verify snapshot checksum ───────────────────────────────────────
        println!("[0/7] Verifying snapshot checksum …");
        verify_checksum(snapshot)?;
        println!("    ✓ Checksum OK");

        if cfg.dry_run {
            println!();
            println!("[dry-run] All import steps skipped — no on-chain writes performed.");
            self.record_history(
                &started_at,
                snapshot.total_records(),
                0,
                MigrationStatus::DryRun,
                None,
                None,
            );
            return Ok(0);
        }

        // ── 1. Backup current target state ────────────────────────────────────
        println!("[1/7] Creating pre-migration backup of target contract …");
        let backup_path = self.create_backup(target)?;
        println!("    ✓ Backup written → {}", backup_path);

        // ── 2. Pause target contract ──────────────────────────────────────────
        println!("[2/7] Pausing target contract …");
        self.rpc
            .invoke_contract(
                target,
                "pause",
                &[
                    ("admin", &cfg.source_contract_id), // admin key from env
                    ("reason", "state-migration-in-progress"),
                ],
            )
            .map_err(|e| format!("Failed to pause target contract: {}", e))?;
        println!("    ✓ Contract paused");

        // ── 3. Import records ─────────────────────────────────────────────────
        println!("[3/7] Importing records …");
        let import_result = self.do_import(snapshot, target);

        match import_result {
            Err(ref e) => {
                eprintln!("    ✗ Import failed: {}", e);
                eprintln!("    Initiating automatic rollback …");
                let rollback_result = self.rollback(&backup_path, target);
                let status = MigrationStatus::RolledBack;
                self.record_history(
                    &started_at,
                    snapshot.total_records(),
                    0,
                    status,
                    Some(e.clone()),
                    Some(backup_path.clone()),
                );
                rollback_result?;
                return Err(format!("Import failed and was rolled back: {}", e));
            }
            Ok(imported) => {
                println!("    ✓ {} records imported", imported);

                // ── 4. Verify ─────────────────────────────────────────────────
                println!("[4/7] Verifying migration …");
                let verifier = VerificationEngine::new(self.config.clone());
                let report = verifier.verify_migration(snapshot, target)?;

                if !report.is_clean() {
                    let err = format!(
                        "Verification failed with {} finding(s). Rolling back.",
                        report.findings.len()
                    );
                    eprintln!("    ✗ {}", err);
                    for f in &report.findings {
                        eprintln!(
                            "      [{:?}] {} — expected={} actual={}",
                            f.severity, f.field, f.expected, f.actual
                        );
                    }
                    let rollback_result = self.rollback(&backup_path, target);
                    self.record_history(
                        &started_at,
                        snapshot.total_records(),
                        imported,
                        MigrationStatus::RolledBack,
                        Some(err.clone()),
                        Some(backup_path.clone()),
                    );
                    rollback_result?;
                    return Err(err);
                }
                println!("    ✓ Verification passed — {} records checked", report.records_checked);

                // ── 5. Unpause ────────────────────────────────────────────────
                println!("[5/7] Unpausing target contract …");
                self.rpc
                    .invoke_contract(
                        target,
                        "unpause",
                        &[("admin", &cfg.source_contract_id)],
                    )
                    .map_err(|e| format!("Failed to unpause target contract: {}", e))?;
                println!("    ✓ Contract unpaused");

                // ── 6. Write verification report ──────────────────────────────
                println!("[6/7] Writing verification report …");
                let report_path = format!(
                    "{}/verification-report.json",
                    cfg.snapshot_dir
                );
                let report_json = serde_json::to_string_pretty(&report)
                    .map_err(|e| format!("Report serialisation error: {}", e))?;
                fs::write(&report_path, report_json)
                    .map_err(|e| format!("Failed to write report: {}", e))?;
                println!("    ✓ Report written → {}", report_path);

                // ── 7. Record history ─────────────────────────────────────────
                println!("[7/7] Recording migration history …");
                self.record_history(
                    &started_at,
                    snapshot.total_records(),
                    imported,
                    MigrationStatus::Success,
                    None,
                    Some(backup_path),
                );
                println!("    ✓ History updated → {}", cfg.history_file);

                println!();
                println!("✓ Migration complete — {} records imported", imported);
                Ok(imported)
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Rollback
    // ─────────────────────────────────────────────────────────────────────────

    /// Restores the target contract to the state captured in `backup_path`.
    ///
    /// This re-imports the backup snapshot and unpauses the contract.
    pub fn rollback(&self, backup_path: &str, target: &str) -> Result<(), String> {
        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║                   ROLLBACK IN PROGRESS                  ║");
        println!("╚══════════════════════════════════════════════════════════╝");
        println!("  Backup : {}", backup_path);

        let json = fs::read_to_string(backup_path)
            .map_err(|e| format!("Cannot read backup {}: {}", backup_path, e))?;
        let backup: StateSnapshot = serde_json::from_str(&json)
            .map_err(|e| format!("Backup parse error: {}", e))?;

        verify_checksum(&backup)?;

        println!("  Restoring {} records …", backup.total_records());
        let restored = self.do_import(&backup, target)?;

        // Unpause after rollback.
        self.rpc
            .invoke_contract(
                target,
                "unpause",
                &[("admin", &self.config.migration.source_contract_id)],
            )
            .map_err(|e| format!("Failed to unpause after rollback: {}", e))?;

        println!("✓ Rollback complete — {} records restored", restored);
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Private helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Writes every record in `snapshot` to `target` via RPC calls.
    /// Returns the number of records written.
    fn do_import(&self, snapshot: &StateSnapshot, target: &str) -> Result<usize, String> {
        let batch = self.config.migration.progress_batch_size;
        let mut count = 0usize;

        // ── instance storage ──────────────────────────────────────────────────
        self.rpc.set_instance_value(target, "Admin", &snapshot.admin)?;
        self.rpc.set_instance_value(target, "FeeBasisPoints", &snapshot.fee_basis_points.to_string())?;
        self.rpc.set_instance_value(target, "RefundWindow", &snapshot.refund_window_seconds.to_string())?;
        self.rpc.set_instance_value(target, "TipCounter", &snapshot.tip_counter.to_string())?;
        self.rpc.set_instance_value(target, "MatchingCounter", &snapshot.matching_counter.to_string())?;
        self.rpc.set_instance_value(target, "CurrentFeeBps", &snapshot.current_fee_bps.to_string())?;
        self.rpc.set_instance_value(target, "ContractVersion", &snapshot.schema_version.to_string())?;
        count += 7;

        for token in &snapshot.whitelisted_tokens {
            self.rpc.whitelist_token(target, token)?;
            count += 1;
        }

        // ── creator balances ──────────────────────────────────────────────────
        for (key, balance) in &snapshot.creator_balances {
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            if parts.len() == 2 {
                self.rpc.set_creator_balance(target, parts[0], parts[1], *balance)?;
                count += 1;
                if count % batch == 0 {
                    println!("    … {} records imported", count);
                }
            }
        }

        // ── creator totals ────────────────────────────────────────────────────
        for (key, total) in &snapshot.creator_totals {
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            if parts.len() == 2 {
                self.rpc.set_creator_total(target, parts[0], parts[1], *total)?;
                count += 1;
                if count % batch == 0 {
                    println!("    … {} records imported", count);
                }
            }
        }

        // ── tip history ───────────────────────────────────────────────────────
        for (creator, tips) in &snapshot.tip_history {
            self.rpc.set_tip_count(target, creator, tips.len() as u64)?;
            for (idx, meta) in tips.iter().enumerate() {
                self.rpc.set_tip_history_entry(target, creator, idx as u64, meta)?;
                count += 1;
                if count % batch == 0 {
                    println!("    … {} records imported", count);
                }
            }
        }

        // ── leaderboard aggregates ────────────────────────────────────────────
        for (key, entry) in &snapshot.tipper_aggregates {
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            if parts.len() == 2 {
                let bucket: u32 = parts[1].parse().unwrap_or(0);
                self.rpc.set_tipper_aggregate(target, parts[0], bucket, entry)?;
                count += 1;
            }
        }
        for (key, entry) in &snapshot.creator_aggregates {
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            if parts.len() == 2 {
                let bucket: u32 = parts[1].parse().unwrap_or(0);
                self.rpc.set_creator_aggregate(target, parts[0], bucket, entry)?;
                count += 1;
            }
        }

        // ── subscriptions ─────────────────────────────────────────────────────
        for sub in snapshot.subscriptions.values() {
            self.rpc.set_subscription(target, sub)?;
            count += 1;
            if count % batch == 0 {
                println!("    … {} records imported", count);
            }
        }

        // ── time locks ────────────────────────────────────────────────────────
        let mut max_lock_id = 0u64;
        for lock in &snapshot.time_locks {
            self.rpc.set_time_lock(target, lock)?;
            if lock.lock_id >= max_lock_id {
                max_lock_id = lock.lock_id + 1;
            }
            count += 1;
        }
        if !snapshot.time_locks.is_empty() {
            self.rpc.set_instance_value(target, "NextLockId", &max_lock_id.to_string())?;
        }

        // ── milestones ────────────────────────────────────────────────────────
        for (key, ms) in &snapshot.milestones {
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            if parts.len() == 2 {
                self.rpc.set_milestone(target, parts[0], ms)?;
                count += 1;
            }
        }

        // ── matching programs ─────────────────────────────────────────────────
        for prog in &snapshot.matching_programs {
            self.rpc.set_matching_program(target, prog)?;
            count += 1;
        }

        // ── tip records ───────────────────────────────────────────────────────
        for rec in &snapshot.tip_records {
            self.rpc.set_tip_record(target, rec)?;
            count += 1;
            if count % batch == 0 {
                println!("    … {} records imported", count);
            }
        }

        // ── locked tips ───────────────────────────────────────────────────────
        for lt in &snapshot.locked_tips {
            self.rpc.set_locked_tip(target, lt)?;
            count += 1;
        }

        // ── user roles ────────────────────────────────────────────────────────
        for (addr, role) in &snapshot.user_roles {
            self.rpc.set_user_role(target, addr, role)?;
            count += 1;
        }

        Ok(count)
    }

    /// Exports the current state of `target` and writes it to the backup path.
    fn create_backup(&self, target: &str) -> Result<String, String> {
        let cfg = &self.config.migration;
        let dir = Path::new(&cfg.snapshot_dir);
        fs::create_dir_all(dir)
            .map_err(|e| format!("Failed to create snapshot dir: {}", e))?;

        // Point the migrator at the target for export, force dry_run=false
        // so the backup is actually written to disk.
        let mut backup_cfg = self.config.clone();
        backup_cfg.migration.dry_run = false;
        backup_cfg.migration.source_contract_id = target.to_string();
        backup_cfg.migration.export_filename = cfg.backup_filename.clone();
        let backup_migrator = StateMigrator::new(backup_cfg);

        backup_migrator.export_state()?;

        let path = dir.join(&cfg.backup_filename);
        Ok(path.to_string_lossy().to_string())
    }

    /// Appends a run entry to the migration history file.
    fn record_history(
        &self,
        started_at: &str,
        records_exported: usize,
        records_imported: usize,
        status: MigrationStatus,
        error: Option<String>,
        backup_path: Option<String>,
    ) {
        let cfg = &self.config.migration;
        let entry = MigrationHistoryEntry {
            started_at: started_at.to_string(),
            finished_at: iso_now(),
            label: cfg.label.clone(),
            source_contract_id: cfg.source_contract_id.clone(),
            target_contract_id: cfg.target_contract_id.clone(),
            network: cfg.network.clone(),
            dry_run: cfg.dry_run,
            status,
            records_exported,
            records_imported,
            error,
            backup_path,
        };
        if let Err(e) = append_history_entry(&cfg.history_file, entry) {
            eprintln!("Warning: failed to write migration history: {}", e);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CLI entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Loads a transformed snapshot from disk and imports it into the target contract.
pub fn run_import(args: &[String]) -> Result<(), String> {
    let config_path = parse_flag(args, "--config")
        .unwrap_or_else(|| "scripts/migrate/config.toml".to_string());
    let input_override = parse_flag(args, "--input");
    let dry_run_override = args.contains(&"--dry-run".to_string());

    let toml_str = fs::read_to_string(&config_path)
        .map_err(|e| format!("Cannot read config {}: {}", config_path, e))?;
    let mut config: MigrationConfig = toml::from_str(&toml_str)
        .map_err(|e| format!("Config parse error: {}", e))?;

    if dry_run_override {
        config.migration.dry_run = true;
    }

    let input_path = input_override.unwrap_or_else(|| {
        format!(
            "{}/{}",
            config.migration.snapshot_dir, config.migration.transformed_filename
        )
    });

    let json = fs::read_to_string(&input_path)
        .map_err(|e| format!("Cannot read snapshot {}: {}", input_path, e))?;
    let snapshot: StateSnapshot = serde_json::from_str(&json)
        .map_err(|e| format!("Snapshot parse error: {}", e))?;

    let migrator = StateMigrator::new(config);
    migrator.import_state(&snapshot)?;
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
