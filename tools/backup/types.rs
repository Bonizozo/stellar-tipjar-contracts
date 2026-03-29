//! Shared types for backup and recovery operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Full snapshot of contract state exported via Stellar RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractState {
    /// Schema version for forward-compatibility checks.
    pub schema_version: u32,
    /// Contract ID (strkey-encoded).
    pub contract_id: String,
    /// Stellar network passphrase the backup was taken from.
    pub network: String,
    /// Ledger sequence at export time.
    pub ledger_sequence: u64,
    /// Ledger timestamp at export time.
    pub ledger_timestamp: u64,
    /// Instance storage entries (key → base64-encoded XDR value).
    pub instance_entries: HashMap<String, String>,
    /// Persistent storage entries (key → base64-encoded XDR value).
    pub persistent_entries: HashMap<String, String>,
    /// Metadata about this backup.
    pub metadata: BackupMetadata,
}

/// Metadata attached to every backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    /// UTC timestamp when the backup was created.
    pub created_at: DateTime<Utc>,
    /// Tool version that produced this backup.
    pub tool_version: String,
    /// SHA-256 hex digest of the JSON payload (excluding this field).
    pub checksum: String,
    /// Whether this is a full or incremental backup.
    pub backup_type: BackupType,
    /// For incremental backups: the backup_id of the base snapshot.
    pub base_backup_id: Option<String>,
    /// Unique identifier for this backup (timestamp + contract suffix).
    pub backup_id: String,
    /// Whether the backup file on disk is encrypted.
    pub encrypted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BackupType {
    Full,
    Incremental,
}

/// Incremental diff between two snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalDiff {
    pub schema_version: u32,
    pub contract_id: String,
    pub base_backup_id: String,
    pub ledger_sequence: u64,
    pub added: HashMap<String, String>,
    pub modified: HashMap<String, String>,
    pub removed: Vec<String>,
    pub metadata: BackupMetadata,
}

/// Result returned after a backup operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupResult {
    pub backup_id: String,
    pub backup_type: BackupType,
    pub storage_location: String,
    pub checksum: String,
    pub encrypted: bool,
    pub size_bytes: u64,
}

/// Result returned after a restore operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct RestoreResult {
    pub backup_id: String,
    pub entries_restored: usize,
    pub verified: bool,
}
