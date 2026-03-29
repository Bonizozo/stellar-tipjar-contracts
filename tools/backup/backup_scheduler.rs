//! backup-scheduler — Run automated periodic backups of TipJar contract state.
//!
//! Usage:
//!   backup-scheduler --contract <CONTRACT_ID> --network <NETWORK>
//!                    --interval <SECONDS> --output-dir <DIR>
//!                    [--encrypt] [--key <HEX_KEY>]
//!                    [--incremental] [--max-backups <N>]
//!                    [--storage s3|ipfs|local]

mod types;
mod crypto;
mod checksum;
mod storage;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Parser;
use std::path::PathBuf;
use std::time::Duration;
use types::{BackupMetadata, BackupResult, BackupType, ContractState};

#[derive(Parser)]
#[command(name = "backup-scheduler", about = "Automated periodic TipJar state backups")]
struct Cli {
    /// Contract ID
    #[arg(long)]
    contract: String,

    /// Network: testnet | mainnet | futurenet
    #[arg(long, default_value = "testnet")]
    network: String,

    /// Backup interval in seconds (default: 3600 = 1 hour)
    #[arg(long, default_value_t = 3600)]
    interval: u64,

    /// Local directory to store backups
    #[arg(long, default_value = "backups")]
    output_dir: PathBuf,

    /// Encrypt backups with AES-256-GCM
    #[arg(long)]
    encrypt: bool,

    /// 32-byte hex encryption key
    #[arg(long)]
    key: Option<String>,

    /// Produce incremental backups after the first full backup
    #[arg(long)]
    incremental: bool,

    /// Maximum number of backups to retain locally (0 = unlimited)
    #[arg(long, default_value_t = 24)]
    max_backups: usize,

    /// Remote storage backend: local | s3 | ipfs
    #[arg(long, default_value = "local")]
    storage: String,

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

/// Invoke the state-exporter binary as a subprocess for each backup cycle.
fn run_exporter(
    contract: &str,
    network: &str,
    rpc: &str,
    output_dir: &PathBuf,
    encrypt: bool,
    key: &Option<String>,
    incremental: bool,
    base_id: Option<&str>,
) -> Result<String> {
    let backup_id = format!(
        "backup_{}_{}",
        &contract[..8.min(contract.len())],
        Utc::now().timestamp()
    );
    let out_path = output_dir.join(format!("{}.json", backup_id));

    let mut cmd = std::process::Command::new("state-exporter");
    cmd.args(["--contract", contract])
        .args(["--network", network])
        .args(["--rpc-url", rpc])
        .args(["--output", &out_path.display().to_string()]);

    if encrypt {
        cmd.arg("--encrypt");
        if let Some(k) = key {
            cmd.args(["--key", k]);
        }
    }
    if incremental {
        if let Some(base) = base_id {
            cmd.arg("--incremental").args(["--base", base]);
        }
    }

    let status = cmd.status().context("Failed to run state-exporter")?;
    anyhow::ensure!(status.success(), "state-exporter exited with error");

    Ok(backup_id)
}

/// Prune old backups beyond `max_backups` in the output directory.
fn prune_old_backups(output_dir: &PathBuf, max_backups: usize) -> Result<()> {
    if max_backups == 0 {
        return Ok(());
    }

    let mut entries: Vec<PathBuf> = std::fs::read_dir(output_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |ext| ext == "json"))
        .collect();

    // Sort by modification time, oldest first.
    entries.sort_by_key(|p| {
        p.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });

    while entries.len() > max_backups {
        let oldest = entries.remove(0);
        println!("[scheduler] Pruning old backup: {}", oldest.display());
        std::fs::remove_file(&oldest)?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let rpc = cli.rpc_url.clone().unwrap_or_else(|| rpc_url_for(&cli.network).to_string());

    std::fs::create_dir_all(&cli.output_dir)?;

    println!("[scheduler] Starting backup scheduler");
    println!("[scheduler]   Contract:  {}", cli.contract);
    println!("[scheduler]   Network:   {}", cli.network);
    println!("[scheduler]   Interval:  {}s", cli.interval);
    println!("[scheduler]   Output:    {}", cli.output_dir.display());
    println!("[scheduler]   Encrypted: {}", cli.encrypt);
    println!("[scheduler]   Incremental: {}", cli.incremental);

    let mut last_backup_id: Option<String> = None;
    let mut cycle = 0u64;

    loop {
        cycle += 1;
        println!("[scheduler] Cycle {} — {}", cycle, Utc::now().to_rfc3339());

        let use_incremental = cli.incremental && last_backup_id.is_some();

        match run_exporter(
            &cli.contract,
            &cli.network,
            &rpc,
            &cli.output_dir,
            cli.encrypt,
            &cli.key,
            use_incremental,
            last_backup_id.as_deref(),
        ) {
            Ok(backup_id) => {
                println!("[scheduler] Backup OK: {}", backup_id);
                last_backup_id = Some(backup_id);
                prune_old_backups(&cli.output_dir, cli.max_backups)?;
            }
            Err(e) => {
                eprintln!("[scheduler] Backup FAILED: {:#}", e);
                // Continue scheduling even on failure.
            }
        }

        tokio::time::sleep(Duration::from_secs(cli.interval)).await;
    }
}
