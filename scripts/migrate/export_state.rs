//! # State Export — `StateMigrator::export_state()`
//!
//! Reads all persistent and instance storage from the **source** TipJar
//! contract and serialises it into a [`StateSnapshot`].
//!
//! ## Dry-run mode
//! When `dry_run = true` in the config the export still runs in full (it is
//! read-only by nature) but the snapshot is **not** written to disk.  This
//! lets operators validate what would be exported without touching the
//! filesystem.
//!
//! ## Progress reporting
//! Progress is printed to stdout after every `config.progress_batch_size`
//! records so long-running exports remain observable.
//!
//! ## Usage
//! ```bash
//! # Export with dry-run (no file written)
//! cargo run --manifest-path scripts/migrate/Cargo.toml \
//!     --bin export -- --config scripts/migrate/config.toml --dry-run
//!
//! # Full export (writes snapshot to disk)
//! cargo run --manifest-path scripts/migrate/Cargo.toml \
//!     --bin export -- --config scripts/migrate/config.toml
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::rpc_client::RpcClient;
use crate::types::{
    LeaderboardEntry, LockedTip, Milestone, MatchingProgram, MigrationConfig, StateSnapshot,
    Subscription, SubscriptionStatus, TipMetadata, TipRecord, TimeLock,
};

// ─────────────────────────────────────────────────────────────────────────────
// StateMigrator
// ─────────────────────────────────────────────────────────────────────────────

/// Orchestrates the export, import, verify, and rollback phases.
pub struct StateMigrator {
    pub config: MigrationConfig,
    pub rpc: RpcClient,
}

impl StateMigrator {
    /// Creates a new migrator from a parsed config.
    pub fn new(config: MigrationConfig) -> Self {
        let rpc = RpcClient::new(
            config.migration.rpc_url.clone(),
            config.migration.max_retries,
            config.migration.retry_delay_ms,
        );
        Self { config, rpc }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // export_state
    // ─────────────────────────────────────────────────────────────────────────

    /// Exports the complete on-chain state of the source contract into a
    /// [`StateSnapshot`].
    ///
    /// # Dry-run behaviour
    /// When `config.migration.dry_run` is `true` the snapshot is returned but
    /// **not** written to disk.
    ///
    /// # Errors
    /// Returns an error string if any RPC call fails after all retries.
    pub fn export_state(&self) -> Result<StateSnapshot, String> {
        let cfg = &self.config.migration;
        let contract_id = &cfg.source_contract_id;

        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║           TipJar State Export — v{:<26}║", cfg.source_version);
        println!("╚══════════════════════════════════════════════════════════╝");
        println!("  Contract : {}", contract_id);
        println!("  Network  : {}", cfg.network);
        println!("  Dry-run  : {}", cfg.dry_run);
        println!();

        // ── instance storage ─────────────────────────────────────────────────
        println!("[1/9] Reading instance storage …");

        let admin = self
            .rpc
            .get_instance_string(contract_id, "Admin")?;
        let fee_basis_points = self
            .rpc
            .get_instance_u32(contract_id, "FeeBasisPoints")
            .unwrap_or(0);
        let refund_window_seconds = self
            .rpc
            .get_instance_u64(contract_id, "RefundWindow")
            .unwrap_or(0);
        let paused = self
            .rpc
            .get_instance_bool(contract_id, "Paused")
            .unwrap_or(false);
        let pause_reason = self
            .rpc
            .get_instance_string_opt(contract_id, "PauseReason");
        let tip_counter = self
            .rpc
            .get_instance_u64(contract_id, "TipCounter")
            .unwrap_or(0);
        let matching_counter = self
            .rpc
            .get_instance_u64(contract_id, "MatchingCounter")
            .unwrap_or(0);
        let current_fee_bps = self
            .rpc
            .get_instance_u32(contract_id, "CurrentFeeBps")
            .unwrap_or(100);
        let schema_version = self
            .rpc
            .get_instance_u32(contract_id, "ContractVersion")
            .unwrap_or(0);
        let whitelisted_tokens = self
            .rpc
            .get_whitelisted_tokens(contract_id)?;

        println!(
            "    admin={}, fee_bps={}, paused={}, tip_counter={}, version={}",
            admin, fee_basis_points, paused, tip_counter, schema_version
        );

        // ── creator balances & totals ─────────────────────────────────────────
        println!("[2/9] Reading creator balances and totals …");
        let (creator_balances, creator_totals) =
            self.export_creator_financials(contract_id)?;
        println!(
            "    {} balance entries, {} total entries",
            creator_balances.len(),
            creator_totals.len()
        );

        // ── tip history ───────────────────────────────────────────────────────
        println!("[3/9] Reading tip history …");
        let tip_history = self.export_tip_history(contract_id, &creator_balances)?;
        let total_tips: usize = tip_history.values().map(|v| v.len()).sum();
        println!(
            "    {} creators with {} total tip records",
            tip_history.len(),
            total_tips
        );

        // ── leaderboard aggregates ────────────────────────────────────────────
        println!("[4/9] Reading leaderboard aggregates …");
        let (tipper_aggregates, creator_aggregates) =
            self.export_leaderboard_aggregates(contract_id)?;
        println!(
            "    {} tipper entries, {} creator entries",
            tipper_aggregates.len(),
            creator_aggregates.len()
        );

        // ── subscriptions ─────────────────────────────────────────────────────
        println!("[5/9] Reading subscriptions …");
        let subscriptions = self.export_subscriptions(contract_id)?;
        println!("    {} subscriptions", subscriptions.len());

        // ── time locks ────────────────────────────────────────────────────────
        println!("[6/9] Reading time-locked tips …");
        let time_locks = self.export_time_locks(contract_id)?;
        println!("    {} time locks", time_locks.len());

        // ── milestones ────────────────────────────────────────────────────────
        println!("[7/9] Reading milestones …");
        let milestones = self.export_milestones(contract_id, &creator_balances)?;
        println!("    {} milestones", milestones.len());

        // ── matching programs ─────────────────────────────────────────────────
        println!("[8/9] Reading matching programs …");
        let matching_programs = self.export_matching_programs(contract_id, matching_counter)?;
        println!("    {} matching programs", matching_programs.len());

        // ── tip records & locked tips ─────────────────────────────────────────
        println!("[9/9] Reading tip records and locked tips …");
        let tip_records = self.export_tip_records(contract_id, tip_counter)?;
        let locked_tips = self.export_locked_tips(contract_id, &creator_balances)?;
        let user_roles = self.export_user_roles(contract_id)?;
        println!(
            "    {} tip records, {} locked tips, {} role assignments",
            tip_records.len(),
            locked_tips.len(),
            user_roles.len()
        );

        // ── ledger metadata ───────────────────────────────────────────────────
        let ledger_sequence = self.rpc.get_latest_ledger_sequence()?;
        let exported_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // ── assemble snapshot ─────────────────────────────────────────────────
        let mut snapshot = StateSnapshot {
            contract_id: contract_id.clone(),
            schema_version,
            exported_at,
            ledger_sequence,
            checksum: String::new(), // filled below
            admin,
            fee_basis_points,
            refund_window_seconds,
            paused,
            pause_reason,
            tip_counter,
            matching_counter,
            current_fee_bps,
            whitelisted_tokens,
            creator_balances,
            creator_totals,
            tip_history,
            tipper_aggregates,
            creator_aggregates,
            subscriptions,
            time_locks,
            milestones,
            matching_programs,
            tip_records,
            locked_tips,
            user_roles,
        };

        // Seal with checksum.
        snapshot.checksum = compute_checksum(&snapshot)?;

        let total = snapshot.total_records();
        println!();
        println!("✓ Export complete — {} total records", total);
        println!("  Checksum : {}", snapshot.checksum);

        // ── persist to disk (skipped in dry-run) ──────────────────────────────
        if cfg.dry_run {
            println!("  [dry-run] Snapshot NOT written to disk.");
        } else {
            let dir = Path::new(&cfg.snapshot_dir);
            fs::create_dir_all(dir)
                .map_err(|e| format!("Failed to create snapshot dir: {}", e))?;
            let path = dir.join(&cfg.export_filename);
            let json = serde_json::to_string_pretty(&snapshot)
                .map_err(|e| format!("Serialisation error: {}", e))?;
            fs::write(&path, json)
                .map_err(|e| format!("Failed to write snapshot: {}", e))?;
            println!("  Snapshot written → {}", path.display());
        }

        Ok(snapshot)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Private helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Reads all `CreatorBalance` and `CreatorTotal` persistent entries.
    fn export_creator_financials(
        &self,
        contract_id: &str,
    ) -> Result<(HashMap<String, i128>, HashMap<String, i128>), String> {
        let batch = self.config.migration.progress_batch_size;
        let raw = self.rpc.scan_persistent_prefix(contract_id, "CreatorBalance")?;
        let mut balances: HashMap<String, i128> = HashMap::new();
        let mut totals: HashMap<String, i128> = HashMap::new();

        for (i, (key, value)) in raw.iter().enumerate() {
            // key format: "CreatorBalance:GCREATOR:GTOKEN"
            let parts: Vec<&str> = key.splitn(3, ':').collect();
            if parts.len() == 3 {
                let composite = format!("{}:{}", parts[1], parts[2]);
                let amount: i128 = value.parse().unwrap_or(0);
                balances.insert(composite, amount);
            }
            if (i + 1) % batch == 0 {
                println!("    … {} balance entries read", i + 1);
            }
        }

        let raw_totals = self.rpc.scan_persistent_prefix(contract_id, "CreatorTotal")?;
        for (i, (key, value)) in raw_totals.iter().enumerate() {
            let parts: Vec<&str> = key.splitn(3, ':').collect();
            if parts.len() == 3 {
                let composite = format!("{}:{}", parts[1], parts[2]);
                let amount: i128 = value.parse().unwrap_or(0);
                totals.insert(composite, amount);
            }
            if (i + 1) % batch == 0 {
                println!("    … {} total entries read", i + 1);
            }
        }

        Ok((balances, totals))
    }

    /// Reads all `TipHistory` and `TipCount` entries for every known creator.
    fn export_tip_history(
        &self,
        contract_id: &str,
        creator_balances: &HashMap<String, i128>,
    ) -> Result<HashMap<String, Vec<TipMetadata>>, String> {
        let batch = self.config.migration.progress_batch_size;
        // Derive the set of known creators from the balance map keys.
        let creators: std::collections::HashSet<String> = creator_balances
            .keys()
            .filter_map(|k| k.split(':').next().map(|s| s.to_string()))
            .collect();

        let mut history: HashMap<String, Vec<TipMetadata>> = HashMap::new();
        let mut total_read = 0usize;

        for creator in &creators {
            let count_key = format!("TipCount:{}", creator);
            let count: u64 = self
                .rpc
                .get_persistent_u64(contract_id, &count_key)
                .unwrap_or(0);

            let mut tips: Vec<TipMetadata> = Vec::with_capacity(count as usize);
            for idx in 0..count {
                let tip_key = format!("TipHistory:{}:{}", creator, idx);
                if let Some(meta) = self.rpc.get_persistent_tip_metadata(contract_id, &tip_key)? {
                    tips.push(meta);
                }
                total_read += 1;
                if total_read % batch == 0 {
                    println!("    … {} tip history records read", total_read);
                }
            }
            if !tips.is_empty() {
                history.insert(creator.clone(), tips);
            }
        }

        Ok(history)
    }

    /// Reads all `TipperAggregate`, `CreatorAggregate`, `TipperParticipants`,
    /// and `CreatorParticipants` entries for bucket 0 (AllTime).
    fn export_leaderboard_aggregates(
        &self,
        contract_id: &str,
    ) -> Result<(HashMap<String, LeaderboardEntry>, HashMap<String, LeaderboardEntry>), String> {
        let batch = self.config.migration.progress_batch_size;
        const BUCKET_ALL_TIME: u32 = 0;

        let tipper_part_key = format!("TipperParticipants:{}", BUCKET_ALL_TIME);
        let tipper_addrs = self
            .rpc
            .get_persistent_address_vec(contract_id, &tipper_part_key)
            .unwrap_or_default();

        let creator_part_key = format!("CreatorParticipants:{}", BUCKET_ALL_TIME);
        let creator_addrs = self
            .rpc
            .get_persistent_address_vec(contract_id, &creator_part_key)
            .unwrap_or_default();

        let mut tipper_agg: HashMap<String, LeaderboardEntry> = HashMap::new();
        for (i, addr) in tipper_addrs.iter().enumerate() {
            let agg_key = format!("TipperAggregate:{}:{}", addr, BUCKET_ALL_TIME);
            if let Some(entry) = self
                .rpc
                .get_persistent_leaderboard_entry(contract_id, &agg_key)?
            {
                tipper_agg.insert(format!("{}:{}", addr, BUCKET_ALL_TIME), entry);
            }
            if (i + 1) % batch == 0 {
                println!("    … {} tipper aggregates read", i + 1);
            }
        }

        let mut creator_agg: HashMap<String, LeaderboardEntry> = HashMap::new();
        for (i, addr) in creator_addrs.iter().enumerate() {
            let agg_key = format!("CreatorAggregate:{}:{}", addr, BUCKET_ALL_TIME);
            if let Some(entry) = self
                .rpc
                .get_persistent_leaderboard_entry(contract_id, &agg_key)?
            {
                creator_agg.insert(format!("{}:{}", addr, BUCKET_ALL_TIME), entry);
            }
            if (i + 1) % batch == 0 {
                println!("    … {} creator aggregates read", i + 1);
            }
        }

        Ok((tipper_agg, creator_agg))
    }

    /// Reads all `Subscription` entries.
    fn export_subscriptions(
        &self,
        contract_id: &str,
    ) -> Result<HashMap<String, Subscription>, String> {
        let batch = self.config.migration.progress_batch_size;
        let raw = self.rpc.scan_persistent_prefix(contract_id, "Subscription")?;
        let mut subs: HashMap<String, Subscription> = HashMap::new();

        for (i, (key, _)) in raw.iter().enumerate() {
            // key: "Subscription:GSUBSCRIBER:GCREATOR"
            let parts: Vec<&str> = key.splitn(3, ':').collect();
            if parts.len() == 3 {
                let sub_key = format!("{}:{}", parts[1], parts[2]);
                if let Some(sub) = self
                    .rpc
                    .get_persistent_subscription(contract_id, key)?
                {
                    subs.insert(sub_key, sub);
                }
            }
            if (i + 1) % batch == 0 {
                println!("    … {} subscriptions read", i + 1);
            }
        }

        Ok(subs)
    }

    /// Reads all `TimeLock` entries via `NextLockId` counter.
    fn export_time_locks(&self, contract_id: &str) -> Result<Vec<TimeLock>, String> {
        let batch = self.config.migration.progress_batch_size;
        let next_id = self
            .rpc
            .get_persistent_u64(contract_id, "NextLockId")
            .unwrap_or(0);

        let mut locks: Vec<TimeLock> = Vec::new();
        for id in 0..next_id {
            let key = format!("TimeLock:{}", id);
            if let Some(lock) = self.rpc.get_persistent_time_lock(contract_id, &key, id)? {
                locks.push(lock);
            }
            if (id as usize + 1) % batch == 0 {
                println!("    … {} time locks read", id + 1);
            }
        }

        Ok(locks)
    }

    /// Reads all `Milestone` entries for every known creator.
    fn export_milestones(
        &self,
        contract_id: &str,
        creator_balances: &HashMap<String, i128>,
    ) -> Result<HashMap<String, Milestone>, String> {
        let batch = self.config.migration.progress_batch_size;
        let creators: std::collections::HashSet<String> = creator_balances
            .keys()
            .filter_map(|k| k.split(':').next().map(|s| s.to_string()))
            .collect();

        let mut milestones: HashMap<String, Milestone> = HashMap::new();
        let mut total_read = 0usize;

        for creator in &creators {
            let counter_key = format!("MilestoneCounter:{}", creator);
            let count: u64 = self
                .rpc
                .get_persistent_u64(contract_id, &counter_key)
                .unwrap_or(0);

            for id in 0..count {
                let key = format!("Milestone:{}:{}", creator, id);
                if let Some(ms) = self.rpc.get_persistent_milestone(contract_id, &key)? {
                    milestones.insert(format!("{}:{}", creator, id), ms);
                }
                total_read += 1;
                if total_read % batch == 0 {
                    println!("    … {} milestones read", total_read);
                }
            }
        }

        Ok(milestones)
    }

    /// Reads all `MatchingProgram` entries via the global counter.
    fn export_matching_programs(
        &self,
        contract_id: &str,
        counter: u64,
    ) -> Result<Vec<MatchingProgram>, String> {
        let batch = self.config.migration.progress_batch_size;
        let mut programs: Vec<MatchingProgram> = Vec::new();

        for id in 0..counter {
            let key = format!("MatchingProgram:{}", id);
            if let Some(prog) = self
                .rpc
                .get_persistent_matching_program(contract_id, &key)?
            {
                programs.push(prog);
            }
            if (id as usize + 1) % batch == 0 {
                println!("    … {} matching programs read", id + 1);
            }
        }

        Ok(programs)
    }

    /// Reads all global `TipRecord` entries via `TipCounter`.
    fn export_tip_records(
        &self,
        contract_id: &str,
        tip_counter: u64,
    ) -> Result<Vec<TipRecord>, String> {
        let batch = self.config.migration.progress_batch_size;
        let mut records: Vec<TipRecord> = Vec::new();

        for id in 0..tip_counter {
            let key = format!("TipRecord:{}", id);
            if let Some(rec) = self.rpc.get_persistent_tip_record(contract_id, &key)? {
                records.push(rec);
            }
            if (id as usize + 1) % batch == 0 {
                println!("    … {} tip records read", id + 1);
            }
        }

        Ok(records)
    }

    /// Reads all `LockedTip` entries for every known creator.
    fn export_locked_tips(
        &self,
        contract_id: &str,
        creator_balances: &HashMap<String, i128>,
    ) -> Result<Vec<LockedTip>, String> {
        let batch = self.config.migration.progress_batch_size;
        let creators: std::collections::HashSet<String> = creator_balances
            .keys()
            .filter_map(|k| k.split(':').next().map(|s| s.to_string()))
            .collect();

        let mut locked: Vec<LockedTip> = Vec::new();
        let mut total_read = 0usize;

        for creator in &creators {
            let counter_key = format!("LockedTipCounter:{}", creator);
            let count: u64 = self
                .rpc
                .get_persistent_u64(contract_id, &counter_key)
                .unwrap_or(0);

            for tip_id in 0..count {
                let key = format!("LockedTip:{}:{}", creator, tip_id);
                if let Some(lt) = self.rpc.get_persistent_locked_tip(contract_id, &key, tip_id)? {
                    locked.push(lt);
                }
                total_read += 1;
                if total_read % batch == 0 {
                    println!("    … {} locked tips read", total_read);
                }
            }
        }

        Ok(locked)
    }

    /// Reads all `UserRole` entries.
    fn export_user_roles(
        &self,
        contract_id: &str,
    ) -> Result<HashMap<String, String>, String> {
        let raw = self.rpc.scan_persistent_prefix(contract_id, "UserRole")?;
        let mut roles: HashMap<String, String> = HashMap::new();

        for (key, value) in raw {
            // key: "UserRole:GADDRESS"
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            if parts.len() == 2 {
                roles.insert(parts[1].to_string(), value);
            }
        }

        Ok(roles)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Checksum helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Computes a SHA-256 checksum over the canonical JSON of the snapshot's data
/// fields (everything except `checksum` itself).
pub fn compute_checksum(snapshot: &StateSnapshot) -> Result<String, String> {
    // Temporarily zero out the checksum field so the hash is deterministic.
    let mut copy = snapshot.clone();
    copy.checksum = String::new();

    let canonical = serde_json::to_string(&copy)
        .map_err(|e| format!("Checksum serialisation error: {}", e))?;

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

/// Verifies that `snapshot.checksum` matches the recomputed checksum.
pub fn verify_checksum(snapshot: &StateSnapshot) -> Result<(), String> {
    let expected = compute_checksum(snapshot)?;
    if snapshot.checksum != expected {
        return Err(format!(
            "Checksum mismatch: stored={}, computed={}",
            snapshot.checksum, expected
        ));
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// CLI entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Parses CLI args, loads config, and runs the export.
pub fn run_export(args: &[String]) -> Result<(), String> {
    let config_path = parse_flag(args, "--config")
        .unwrap_or_else(|| "scripts/migrate/config.toml".to_string());
    let dry_run_override = args.contains(&"--dry-run".to_string());

    let toml_str = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Cannot read config {}: {}", config_path, e))?;
    let mut config: MigrationConfig = toml::from_str(&toml_str)
        .map_err(|e| format!("Config parse error: {}", e))?;

    if dry_run_override {
        config.migration.dry_run = true;
    }

    let migrator = StateMigrator::new(config);
    migrator.export_state()?;
    Ok(())
}

fn parse_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
}
