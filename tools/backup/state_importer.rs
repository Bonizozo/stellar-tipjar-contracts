//! state-importer — Restore TipJar contract state from a JSON backup.
//!
//! Usage:
//!   state-importer --backup <FILE_OR_URI> --contract <CONTRACT_ID>
//!                  --network <NETWORK> --source <ACCOUNT>
//!                  [--decrypt] [--key <HEX_KEY>] [--dry-run] [--verify-only]

mod types;
mod crypto;
mod checksum;
mod storage;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use types::{BackupType, ContractState, IncrementalDiff, RestoreResult};

#[derive(Parser)]
#[command(name = "state-importer", about = "Restore TipJar contract state from a backup")]
struct Cli {
    /// Path or URI (s3:// / ipfs://) to the backup file
    #[arg(long)]
    backup: String,

    /// Contract ID to restore into
    #[arg(long)]
    contract: String,

    /// Network: testnet | mainnet | futurenet
    #[arg(long, default_value = "testnet")]
    network: String,

    /// Stellar account to sign restore transactions
    #[arg(long)]
    source: String,

    /// Decrypt the backup before importing
    #[arg(long)]
    decrypt: bool,

    /// 32-byte hex decryption key
    #[arg(long)]
    key: Option<String>,

    /// Print what would be restored without submitting transactions
    #[arg(long)]
    dry_run: bool,

    /// Only verify backup integrity, do not restore
    #[arg(long)]
    verify_only: bool,

    /// RPC endpoint override
    #[arg(long)]
    rpc_url: Option<String>,
}

fn rpc_url_for(network: &str) -> &'static str {
    match network {
        "mainnet" => "https://mainnet.sorobanrpc.com",
        "futurenet" => "https://rpc-futurenet.stellar.org",
        _ => "https://soroban-testnet.stellar.org",
    }
}

/// Load and optionally decrypt a backup file from a local path or URI.
async fn load_backup_bytes(backup: &str, decrypt: bool, key: &Option<String>) -> Result<Vec<u8>> {
    let raw = if backup.starts_with("s3://") || backup.starts_with("ipfs://") {
        // Determine backend from URI scheme.
        let backend = if backup.starts_with("s3://") {
            storage::StorageBackend::S3(storage::S3Config {
                bucket: String::new(),
                region: std::env::var("AWS_DEFAULT_REGION").unwrap_or_else(|_| "us-east-1".into()),
                prefix: String::new(),
            })
        } else {
            storage::StorageBackend::Ipfs(storage::IpfsConfig {
                api_url: std::env::var("IPFS_API_URL")
                    .unwrap_or_else(|_| "http://localhost:5001".into()),
            })
        };
        storage::download(backup, &backend).await?
    } else {
        std::fs::read(backup).with_context(|| format!("Cannot read backup file: {}", backup))?
    };

    if decrypt {
        let key_bytes = resolve_key(key)?;
        crypto::decrypt(&raw, &key_bytes)
    } else {
        Ok(raw)
    }
}

/// Verify backup integrity by recomputing the SHA-256 checksum.
fn verify_backup(state: &ContractState) -> Result<bool> {
    // Re-serialise with an empty checksum field to reproduce the original payload.
    let mut clone = state.clone();
    let stored_checksum = clone.metadata.checksum.clone();
    clone.metadata.checksum = String::new();

    let json = serde_json::to_string_pretty(&clone)?;
    let computed = checksum::sha256_hex(json.as_bytes());

    if computed == stored_checksum {
        println!("[importer] Checksum OK: {}", computed);
        Ok(true)
    } else {
        println!("[importer] CHECKSUM MISMATCH!");
        println!("[importer]   stored:   {}", stored_checksum);
        println!("[importer]   computed: {}", computed);
        Ok(false)
    }
}

/// Submit a contract invocation to restore a single storage entry.
async fn restore_entry(
    rpc: &str,
    contract_id: &str,
    source: &str,
    key_xdr: &str,
    value_xdr: &str,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("[importer][dry-run] Would restore key: {}...", &key_xdr[..key_xdr.len().min(40)]);
        return Ok(());
    }

    // In production this would build and submit a Stellar transaction that calls
    // a privileged `restore_entry` contract function (admin-only) or uses the
    // Stellar CLI `contract invoke` with the appropriate XDR arguments.
    // Here we shell out to the Stellar CLI for simplicity.
    let status = std::process::Command::new("stellar")
        .args([
            "contract", "invoke",
            "--id", contract_id,
            "--source", source,
            "--network", rpc,
            "--",
            "restore_entry",
            "--key", key_xdr,
            "--value", value_xdr,
        ])
        .status()
        .context("Failed to invoke stellar CLI")?;

    anyhow::ensure!(status.success(), "stellar contract invoke failed for key: {}", key_xdr);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let rpc = cli.rpc_url.as_deref().unwrap_or_else(|| rpc_url_for(&cli.network));

    println!("[importer] Loading backup: {}", cli.backup);
    let bytes = load_backup_bytes(&cli.backup, cli.decrypt, &cli.key).await?;

    // Try to parse as full ContractState first, then as IncrementalDiff.
    let (entries, backup_id, is_incremental) = if let Ok(state) =
        serde_json::from_slice::<ContractState>(&bytes)
    {
        println!("[importer] Backup type: full");
        println!("[importer] Contract:    {}", state.contract_id);
        println!("[importer] Network:     {}", state.network);
        println!("[importer] Ledger:      {}", state.ledger_sequence);

        let verified = verify_backup(&state)?;
        if !verified {
            anyhow::bail!("Backup integrity check failed — aborting restore");
        }
        if cli.verify_only {
            println!("[importer] --verify-only: done.");
            return Ok(());
        }

        let mut all: Vec<(String, String)> = Vec::new();
        all.extend(state.instance_entries.iter().map(|(k, v)| (k.clone(), v.clone())));
        all.extend(state.persistent_entries.iter().map(|(k, v)| (k.clone(), v.clone())));
        (all, state.metadata.backup_id.clone(), false)
    } else {
        let diff: IncrementalDiff = serde_json::from_slice(&bytes)
            .context("Failed to parse backup as ContractState or IncrementalDiff")?;

        println!("[importer] Backup type: incremental (base: {})", diff.base_backup_id);
        if cli.verify_only {
            println!("[importer] --verify-only: incremental diff loaded OK.");
            return Ok(());
        }

        let mut all: Vec<(String, String)> = Vec::new();
        all.extend(diff.added.iter().map(|(k, v)| (k.clone(), v.clone())));
        all.extend(diff.modified.iter().map(|(k, v)| (k.clone(), v.clone())));
        // Removed entries are not re-applied in an incremental restore.
        (all, diff.metadata.backup_id.clone(), true)
    };

    println!("[importer] Restoring {} entries (dry_run={}, incremental={}) ...",
        entries.len(), cli.dry_run, is_incremental);

    let mut restored = 0usize;
    for (key, value) in &entries {
        restore_entry(rpc, &cli.contract, &cli.source, key, value, cli.dry_run).await?;
        restored += 1;
    }

    let result = RestoreResult {
        backup_id,
        entries_restored: restored,
        verified: true,
    };

    println!("[importer] {}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn resolve_key(opt: &Option<String>) -> Result<[u8; 32]> {
    let hex_key = opt.as_deref().context("--key required for decryption")?;
    let bytes = hex::decode(hex_key).context("Invalid hex key")?;
    anyhow::ensure!(bytes.len() == 32, "Key must be 32 bytes (64 hex chars)");
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}
