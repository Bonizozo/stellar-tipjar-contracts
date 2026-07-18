//! Shared types for the TipJar contract state migration toolkit.
//!
//! All types here are serialisable with serde so they can be written to / read
//! from the JSON snapshot files that travel between the export, transform,
//! import, and verify phases.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Mirror types (off-chain representations of on-chain contracttype structs)
// ─────────────────────────────────────────────────────────────────────────────

/// Off-chain mirror of the on-chain `TipMetadata` contracttype.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TipMetadata {
    pub sender: String,
    pub amount: i128,
    pub message: Option<String>,
    pub timestamp: u64,
}

/// Off-chain mirror of the on-chain `LeaderboardEntry` contracttype.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LeaderboardEntry {
    pub address: String,
    pub total_amount: i128,
    pub tip_count: u32,
}

/// Off-chain mirror of the on-chain `Subscription` contracttype.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Subscription {
    pub subscriber: String,
    pub creator: String,
    pub token: String,
    pub amount: i128,
    pub interval_seconds: u64,
    pub last_payment: u64,
    pub next_payment: u64,
    pub status: SubscriptionStatus,
}

/// Off-chain mirror of `SubscriptionStatus`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    Paused,
    Cancelled,
}

/// Off-chain mirror of the on-chain `TimeLock` contracttype.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeLock {
    pub lock_id: u64,
    pub sender: String,
    pub creator: String,
    pub token: String,
    pub amount: i128,
    pub unlock_time: u64,
    pub cancelled: bool,
}

/// Off-chain mirror of the on-chain `Milestone` contracttype.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Milestone {
    pub id: u64,
    pub creator: String,
    pub goal_amount: i128,
    pub current_amount: i128,
    pub description: String,
    pub deadline: Option<u64>,
    pub completed: bool,
}

/// Off-chain mirror of the on-chain `MatchingProgram` contracttype.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchingProgram {
    pub id: u64,
    pub sponsor: String,
    pub creator: String,
    pub token: String,
    pub match_ratio: u32,
    pub max_match_amount: i128,
    pub current_matched: i128,
    pub active: bool,
}

/// Off-chain mirror of the on-chain `TipRecord` contracttype.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TipRecord {
    pub id: u64,
    pub sender: String,
    pub creator: String,
    pub token: String,
    pub amount: i128,
    pub timestamp: u64,
    pub refunded: bool,
    pub refund_requested: bool,
    pub refund_approved: bool,
}

/// Off-chain mirror of the on-chain `LockedTip` contracttype.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockedTip {
    pub tip_id: u64,
    pub sender: String,
    pub creator: String,
    pub token: String,
    pub amount: i128,
    pub unlock_timestamp: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Snapshot — the complete exported state of one contract version
// ─────────────────────────────────────────────────────────────────────────────

/// Complete on-chain state snapshot for one contract version.
///
/// Produced by `export_state`, consumed by `transform_state` and `import_state`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    // ── metadata ─────────────────────────────────────────────────────────────
    /// Contract address this snapshot was taken from.
    pub contract_id: String,
    /// On-chain `ContractVersion` at export time.
    pub schema_version: u32,
    /// Unix timestamp (seconds) when the snapshot was created.
    pub exported_at: u64,
    /// Ledger sequence number at export time.
    pub ledger_sequence: u32,
    /// SHA-256 hex digest of the canonical JSON of all data fields below.
    /// Computed and verified by the toolkit; empty string before sealing.
    pub checksum: String,

    // ── instance storage ─────────────────────────────────────────────────────
    /// Admin address.
    pub admin: String,
    /// Fee in basis points (0–500).
    pub fee_basis_points: u32,
    /// Refund window in seconds.
    pub refund_window_seconds: u64,
    /// Whether the contract is currently paused.
    pub paused: bool,
    /// Human-readable pause reason (empty when not paused).
    pub pause_reason: Option<String>,
    /// Global tip counter.
    pub tip_counter: u64,
    /// Global matching program counter.
    pub matching_counter: u64,
    /// Most-recently computed dynamic fee in basis points.
    pub current_fee_bps: u32,
    /// Whitelisted token addresses.
    pub whitelisted_tokens: Vec<String>,

    // ── persistent storage ────────────────────────────────────────────────────
    /// Creator balances: key = "creator_address:token_address", value = balance.
    pub creator_balances: HashMap<String, i128>,
    /// Creator historical totals: key = "creator_address:token_address", value = total.
    pub creator_totals: HashMap<String, i128>,
    /// Tip history per creator: key = creator_address, value = ordered list of metadata.
    pub tip_history: HashMap<String, Vec<TipMetadata>>,
    /// Leaderboard aggregates for tippers: key = "address:bucket", value = entry.
    pub tipper_aggregates: HashMap<String, LeaderboardEntry>,
    /// Leaderboard aggregates for creators: key = "address:bucket", value = entry.
    pub creator_aggregates: HashMap<String, LeaderboardEntry>,
    /// Subscriptions: key = "subscriber:creator", value = subscription.
    pub subscriptions: HashMap<String, Subscription>,
    /// Time-locked tips.
    pub time_locks: Vec<TimeLock>,
    /// Milestones: key = "creator:milestone_id", value = milestone.
    pub milestones: HashMap<String, Milestone>,
    /// Matching programs.
    pub matching_programs: Vec<MatchingProgram>,
    /// Global tip records.
    pub tip_records: Vec<TipRecord>,
    /// Locked tips.
    pub locked_tips: Vec<LockedTip>,
    /// Role assignments: key = address, value = role string.
    pub user_roles: HashMap<String, String>,
}

impl StateSnapshot {
    /// Returns the total number of data records across all collections.
    pub fn total_records(&self) -> usize {
        self.creator_balances.len()
            + self.creator_totals.len()
            + self.tip_history.values().map(|v| v.len()).sum::<usize>()
            + self.tipper_aggregates.len()
            + self.creator_aggregates.len()
            + self.subscriptions.len()
            + self.time_locks.len()
            + self.milestones.len()
            + self.matching_programs.len()
            + self.tip_records.len()
            + self.locked_tips.len()
            + self.user_roles.len()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Migration configuration (parsed from config.toml)
// ─────────────────────────────────────────────────────────────────────────────

/// Parsed representation of `config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    pub migration: MigrationSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationSection {
    pub label: String,
    pub source_contract_id: String,
    pub target_contract_id: String,
    pub network: String,
    pub rpc_url: String,
    pub network_passphrase: String,
    pub dry_run: bool,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub snapshot_dir: String,
    pub backup_filename: String,
    pub export_filename: String,
    pub transformed_filename: String,
    pub history_file: String,
    pub source_version: u32,
    pub target_version: u32,
    pub progress_batch_size: usize,
    pub abort_on_verify_failure: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Migration history
// ─────────────────────────────────────────────────────────────────────────────

/// Status of a single migration run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationStatus {
    Success,
    Failed,
    RolledBack,
    DryRun,
}

/// One entry in the migration history log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationHistoryEntry {
    /// ISO-8601 timestamp of when the run started.
    pub started_at: String,
    /// ISO-8601 timestamp of when the run finished (or failed).
    pub finished_at: String,
    /// Human-readable label from config.
    pub label: String,
    /// Source contract ID.
    pub source_contract_id: String,
    /// Target contract ID.
    pub target_contract_id: String,
    /// Network used.
    pub network: String,
    /// Whether this was a dry run.
    pub dry_run: bool,
    /// Final status.
    pub status: MigrationStatus,
    /// Number of records exported.
    pub records_exported: usize,
    /// Number of records imported (0 for dry runs).
    pub records_imported: usize,
    /// Error message if status is Failed or RolledBack.
    pub error: Option<String>,
    /// Path to the backup snapshot file.
    pub backup_path: Option<String>,
}

/// The full history log file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MigrationHistory {
    pub runs: Vec<MigrationHistoryEntry>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Verification report
// ─────────────────────────────────────────────────────────────────────────────

/// Severity of a single verification finding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    /// Data is present in source but missing in target.
    Missing,
    /// Data is present in target but not in source (unexpected extra).
    Extra,
    /// Data exists in both but values differ.
    Mismatch,
}

/// A single field-level discrepancy found during verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationFinding {
    pub severity: FindingSeverity,
    /// Dot-path to the field, e.g. `"creator_balances.GABC…:GDEX…"`.
    pub field: String,
    /// Stringified expected value (from source snapshot).
    pub expected: String,
    /// Stringified actual value (from target contract), or `"<absent>"`.
    pub actual: String,
}

/// Full verification report returned by `verify_migration`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub source_contract_id: String,
    pub target_contract_id: String,
    pub verified_at: String,
    pub records_checked: usize,
    pub passed: bool,
    pub findings: Vec<VerificationFinding>,
}

impl VerificationReport {
    /// Returns `true` when there are no findings.
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Transformation log
// ─────────────────────────────────────────────────────────────────────────────

/// One logged transformation applied during the transform phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformationLog {
    /// Short identifier for the transformation rule, e.g. `"add_field_v2_metadata"`.
    pub rule: String,
    /// Human-readable description.
    pub description: String,
    /// Number of records affected.
    pub records_affected: usize,
}
