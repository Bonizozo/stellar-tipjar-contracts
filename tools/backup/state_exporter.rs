//! state-exporter — Export TipJar contract state to a JSON backup file.
//!
//! Usage:
//!   state-exporter --contract <CONTRACT_ID> --network <NETWORK> [--output <FILE>]
//!                  [--encrypt] [--key <HEX_KEY>] [--incremental --base <BACKUP_ID>]

mod types;
mod crypto;
mod storage;
mod checksum;

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::Utc;
use clap::Parser;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use types::{BackupMetadata, BackupResult, BackupType, ContractState, IncrementalDiff};

#[derive(Parser)]
#[command(name = "state-exporter", about = "Export TipJar contract state to JSON")]
struct Cli {
    /// Contract ID (strkey C... format)
    #[arg(long)]
    contract: String,

    /// Network: testnet | mainnet | futurenet
    #[arg(long, default_value = "testnet")]
    network: String,

    /// Output file path (defaults to <backup_id>.json)
    #[arg(long)]
    output: Option<PathBuf>,

    /// Encrypt the backup with AES-256-GCM
    #[arg(long)]
    encrypt: bool,

    /// 32-byte hex encryption key (generated if omitted and --encrypt is set)
    #[arg(long)]
    key: Option<String>,

    /// Produce an incremental backup against a base snapshot
    #[arg(long)]
    incremental: bool,

    /// Base backup ID for incremental diff
    #[arg(long)]
    base: Option<String>,

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

/// Fetch all ledger entries for a contract via Stellar RPC getLedgerEntries.
/// Returns (instance_entries, persistent_entries, ledger_sequence, ledger_timestamp).
async fn fetch_contract_entries(
    rpc: &str,
    contract_id: &str,
) -> Result<(HashMap<String, String>, HashMap<String, String>, u64, u64)> {
    let client = reqwest::Client::new();

    // First, get the contract instance entry to discover the ledger sequence.
    let instance_key = build_contract_instance_key(contract_id)?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getLedgerEntries",
        "params": { "keys": [instance_key] }
    });

    let resp: Value = client
        .post(rpc)
        .json(&body)
        .send()
        .await
        .context("RPC request failed")?
        .json()
        .await
        .context("Failed to parse RPC response")?;

    let ledger_sequence = resp["result"]["latestLedger"]
        .as_u64()
        .unwrap_or(0);

    let ledger_timestamp = resp["result"]["latestLedgerCloseTime"]
        .as_u64()
        .unwrap_or(0);

    // Parse instance entries from the response.
    let mut instance_entries: HashMap<String, String> = HashMap::new();
    let mut persistent_entries: HashMap<String, String> = HashMap::new();

    if let Some(entries) = resp["result"]["entries"].as_array() {
        for entry in entries {
            let key = entry["key"].as_str().unwrap_or("").to_string();
            let xdr = entry["xdr"].as_str().unwrap_or("").to_string();
            // Instance entries contain the contract data map; we store raw XDR.
            instance_entries.insert(key, xdr);
        }
    }

    // Fetch persistent storage entries via getContractData (paginated).
    let persistent = fetch_persistent_entries(&client, rpc, contract_id).await?;
    persistent_entries.extend(persistent);

    Ok((instance_entries, persistent_entries, ledger_sequence, ledger_timestamp))
}

/// Fetch persistent contract data entries using getLedgerEntries with contract data keys.
async fn fetch_persistent_entries(
    client: &reqwest::Client,
    rpc: &str,
    contract_id: &str,
) -> Result<HashMap<String, String>> {
    let mut entries: HashMap<String, String> = HashMap::new();

    // Use getContractData RPC method to enumerate all persistent entries.
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "getContractData",
        "params": {
            "contract": contract_id,
            "durability": "persistent"
        }
    });

    let resp: Value = client
        .post(rpc)
        .json(&body)
        .send()
        .await
        .context("getContractData RPC request failed")?
        .json()
        .await
        .context("Failed to parse getContractData response")?;

    if let Some(data_entries) = resp["result"]["entries"].as_array() {
        for entry in data_entries {
            let key = entry["key"].as_str().unwrap_or("").to_string();
            let val = entry["value"].as_str().unwrap_or("").to_string();
            entries.insert(key, val);
        }
    }

    Ok(entries)
}

/// Build the XDR key for a contract's instance storage entry (base64-encoded).
fn build_contract_instance_key(contract_id: &str) -> Result<String> {
    // The instance key is a LedgerKey of type CONTRACT_DATA with the contract address
    // and key = ScVal::LedgerKeyContractInstance. We encode a well-known placeholder
    // and let the RPC resolve it. In practice, callers use stellar-xdr or the SDK.
    // Here we return the base64 of the canonical XDR for the instance key.
    // This is a simplified representation; production code should use stellar-xdr crate.
    let placeholder = format!("instance:{}", contract_id);
    Ok(B64.encode(placeholder.as_bytes()))
}

/// Compute incremental diff between a base snapshot and current entries.
fn compute_diff(
    base: &ContractState,
    current_instance: &HashMap<String, String>,
    current_persistent: &HashMap<String, String>,
) -> (HashMap<String, String>, HashMap<String, String>, Vec<String>) {
    let mut added = HashMap::new();
    let mut modified = HashMap::new();
    let mut removed = Vec::new();

    let mut all_current: HashMap<String, String> = HashMap::new();
    all_current.extend(current_instance.clone());
    all_current.extend(current_persistent.clone());

    let mut all_base: HashMap<String, String> = HashMap::new();
    all_base.extend(base.instance_entries.clone());
    all_base.extend(base.persistent_entries.clone());

    for (k, v) in &all_current {
        match all_base.get(k) {
            None => { added.insert(k.clone(), v.clone()); }
            Some(old) if old != v => { modified.insert(k.clone(), v.clone()); }
            _ => {}
        }
    }
    for k in all_base.keys() {
        if !all_current.contains_key(k) {
            removed.push(k.clone());
        }
    }

    (added, modified, removed)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let rpc = cli.rpc_url.as_deref().unwrap_or_else(|| rpc_url_for(&cli.network));

    println!("[exporter] Fetching contract state from {} ...", rpc);
    let (instance_entries, persistent_entries, ledger_sequence, ledger_timestamp) =
        fetch_contract_entries(rpc, &cli.contract).await?;

    let backup_id = format!(
        "backup_{}_{}",
        &cli.contract[..8.min(cli.contract.len())],
        Utc::now().timestamp()
    );

    let total_entries = instance_entries.len() + persistent_entries.len();
    println!("[exporter] Fetched {} entries (instance={}, persistent={})",
        total_entries, instance_entries.len(), persistent_entries.len());

    // Build the state struct (checksum computed after serialisation).
    let mut state = ContractState {
        schema_version: 1,
        contract_id: cli.contract.clone(),
        network: cli.network.clone(),
        ledger_sequence,
        ledger_timestamp,
        instance_entries: instance_entries.clone(),
        persistent_entries: persistent_entries.clone(),
        metadata: BackupMetadata {
            created_at: Utc::now(),
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            checksum: String::new(), // filled below
            backup_type: if cli.incremental { BackupType::Incremental } else { BackupType::Full },
            base_backup_id: cli.base.clone(),
            backup_id: backup_id.clone(),
            encrypted: cli.encrypt,
        },
    };

    // Handle incremental mode.
    if cli.incremental {
        let base_id = cli.base.as_deref().context("--base required for incremental backup")?;
        let base_path = PathBuf::from(format!("{}.json", base_id));
        let base_json = std::fs::read_to_string(&base_path)
            .with_context(|| format!("Cannot read base backup: {}", base_path.display()))?;
        let base_state: ContractState = serde_json::from_str(&base_json)?;

        let (added, modified, removed) =
            compute_diff(&base_state, &instance_entries, &persistent_entries);

        let diff = IncrementalDiff {
            schema_version: 1,
            contract_id: cli.contract.clone(),
            base_backup_id: base_id.to_string(),
            ledger_sequence,
            added,
            modified,
            removed,
            metadata: state.metadata.clone(),
        };

        let json_for_hash = serde_json::to_string_pretty(&diff)?;
        let checksum = checksum::sha256_hex(json_for_hash.as_bytes());
        // Re-embed checksum: clone diff with checksum set, then re-serialise.
        let mut diff_with_checksum = diff;
        diff_with_checksum.metadata.checksum = checksum.clone();
        let json = serde_json::to_string_pretty(&diff_with_checksum)?;

        let out_path = cli.output.unwrap_or_else(|| PathBuf::from(format!("{}.json", backup_id)));
        let (final_bytes, encrypted) = if cli.encrypt {
            let key = resolve_key(&cli.key)?;
            (crypto::encrypt(json.as_bytes(), &key)?, true)
        } else {
            (json.into_bytes(), false)
        };

        std::fs::write(&out_path, &final_bytes)?;
        println!("[exporter] Incremental backup written to {}", out_path.display());
        println!("[exporter] Checksum: {}", checksum);
        println!("[exporter] Encrypted: {}", encrypted);
        return Ok(());
    }

    // Full backup: checksum is computed over the payload with checksum="" (same
    // convention used by verify_backup in the importer), then embedded.
    // state.metadata.checksum is already "" here, so we serialise, hash, embed,
    // then re-serialise once more so the file contains the correct digest.
    let json_for_hash = serde_json::to_string_pretty(&state)?;
    let checksum = checksum::sha256_hex(json_for_hash.as_bytes());
    state.metadata.checksum = checksum.clone();
    let json = serde_json::to_string_pretty(&state)?;

    let out_path = cli.output.unwrap_or_else(|| PathBuf::from(format!("{}.json", backup_id)));

    let (final_bytes, encrypted) = if cli.encrypt {
        let key = resolve_key(&cli.key)?;
        let ciphertext = crypto::encrypt(json.as_bytes(), &key)?;
        (ciphertext, true)
    } else {
        (json.into_bytes(), false)
    };

    let size_bytes = final_bytes.len() as u64;
    std::fs::write(&out_path, &final_bytes)?;

    let result = BackupResult {
        backup_id: backup_id.clone(),
        backup_type: BackupType::Full,
        storage_location: out_path.display().to_string(),
        checksum,
        encrypted,
        size_bytes,
    };

    println!("[exporter] {}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

/// Resolve or generate a 32-byte AES key from the CLI option.
fn resolve_key(opt: &Option<String>) -> Result<[u8; 32]> {
    match opt {
        Some(hex_key) => {
            let bytes = hex::decode(hex_key).context("Invalid hex key")?;
            anyhow::ensure!(bytes.len() == 32, "Key must be 32 bytes (64 hex chars)");
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            Ok(arr)
        }
        None => {
            let key: [u8; 32] = rand::random();
            println!("[exporter] Generated encryption key: {}", hex::encode(key));
            println!("[exporter] SAVE THIS KEY — it is required for decryption.");
            Ok(key)
        }
    }
}
