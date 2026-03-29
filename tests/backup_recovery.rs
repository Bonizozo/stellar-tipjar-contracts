//! Tests for contract state backup and recovery logic.
//!
//! Exercises backup types, checksum verification, encryption,
//! incremental diff logic, and integrity checks — no live network required.

use std::collections::HashMap;
use chrono::Utc;

// ── Re-use the backup types inline (no workspace dep on tools/backup) ─────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct BackupMetadata {
    created_at: chrono::DateTime<Utc>,
    tool_version: String,
    checksum: String,
    backup_type: BackupType,
    base_backup_id: Option<String>,
    backup_id: String,
    encrypted: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum BackupType { Full, Incremental }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ContractState {
    schema_version: u32,
    contract_id: String,
    network: String,
    ledger_sequence: u64,
    ledger_timestamp: u64,
    instance_entries: HashMap<String, String>,
    persistent_entries: HashMap<String, String>,
    metadata: BackupMetadata,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct IncrementalDiff {
    schema_version: u32,
    contract_id: String,
    base_backup_id: String,
    ledger_sequence: u64,
    added: HashMap<String, String>,
    modified: HashMap<String, String>,
    removed: Vec<String>,
    metadata: BackupMetadata,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

fn make_state(contract_id: &str, instance: HashMap<String, String>, persistent: HashMap<String, String>) -> ContractState {
    let backup_id = format!("backup_test_{}", Utc::now().timestamp());
    let mut state = ContractState {
        schema_version: 1,
        contract_id: contract_id.to_string(),
        network: "testnet".to_string(),
        ledger_sequence: 1000,
        ledger_timestamp: 1_700_000_000,
        instance_entries: instance,
        persistent_entries: persistent,
        metadata: BackupMetadata {
            created_at: Utc::now(),
            tool_version: "0.1.0".to_string(),
            checksum: String::new(),
            backup_type: BackupType::Full,
            base_backup_id: None,
            backup_id: backup_id.clone(),
            encrypted: false,
        },
    };
    // Compute and embed checksum.
    let json = serde_json::to_string_pretty(&state).unwrap();
    state.metadata.checksum = sha256_hex(json.as_bytes());
    state
}

fn verify_state_checksum(state: &ContractState) -> bool {
    let mut clone = state.clone();
    let stored = clone.metadata.checksum.clone();
    clone.metadata.checksum = String::new();
    let json = serde_json::to_string_pretty(&clone).unwrap();
    sha256_hex(json.as_bytes()) == stored
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_state_export_completeness() {
    let mut instance = HashMap::new();
    instance.insert("Admin".to_string(), "GABC123".to_string());
    instance.insert("Paused".to_string(), "false".to_string());

    let mut persistent = HashMap::new();
    persistent.insert("CreatorBalance(GCREATOR1,TOKEN1)".to_string(), "1000000".to_string());
    persistent.insert("CreatorTotal(GCREATOR1,TOKEN1)".to_string(), "5000000".to_string());
    persistent.insert("TipCounter".to_string(), "42".to_string());

    let state = make_state("CCONTRACT123", instance.clone(), persistent.clone());

    assert_eq!(state.schema_version, 1);
    assert_eq!(state.contract_id, "CCONTRACT123");
    assert_eq!(state.instance_entries.len(), instance.len());
    assert_eq!(state.persistent_entries.len(), persistent.len());
    assert!(state.instance_entries.contains_key("Admin"));
    assert!(state.persistent_entries.contains_key("TipCounter"));
}

#[test]
fn test_backup_checksum_valid() {
    let state = make_state("CCONTRACT123", HashMap::new(), HashMap::new());
    assert!(verify_state_checksum(&state), "Checksum should be valid for a freshly created backup");
}

#[test]
fn test_backup_checksum_detects_tampering() {
    let mut state = make_state("CCONTRACT123", HashMap::new(), HashMap::new());
    // Tamper with a field after checksum was computed.
    state.ledger_sequence = 9999;
    assert!(!verify_state_checksum(&state), "Checksum should fail after tampering");
}

#[test]
fn test_backup_serialization_roundtrip() {
    let mut persistent = HashMap::new();
    persistent.insert("CreatorBalance(GCREATOR,TOKEN)".to_string(), "500".to_string());

    let state = make_state("CTEST", HashMap::new(), persistent);
    let json = serde_json::to_string_pretty(&state).unwrap();
    let restored: ContractState = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.contract_id, state.contract_id);
    assert_eq!(restored.persistent_entries, state.persistent_entries);
    assert_eq!(restored.metadata.backup_id, state.metadata.backup_id);
}

#[test]
fn test_incremental_diff_added_entries() {
    let base = make_state("CTEST", HashMap::new(), HashMap::new());

    let mut new_persistent = HashMap::new();
    new_persistent.insert("CreatorBalance(GNEW,TOKEN)".to_string(), "100".to_string());

    let diff = compute_diff(&base, &HashMap::new(), &new_persistent);
    assert_eq!(diff.0.len(), 1, "Should detect 1 added entry");
    assert!(diff.0.contains_key("CreatorBalance(GNEW,TOKEN)"));
    assert!(diff.1.is_empty(), "No modified entries");
    assert!(diff.2.is_empty(), "No removed entries");
}

#[test]
fn test_incremental_diff_modified_entries() {
    let mut persistent = HashMap::new();
    persistent.insert("CreatorBalance(GCREATOR,TOKEN)".to_string(), "100".to_string());
    let base = make_state("CTEST", HashMap::new(), persistent);

    let mut new_persistent = HashMap::new();
    new_persistent.insert("CreatorBalance(GCREATOR,TOKEN)".to_string(), "200".to_string());

    let diff = compute_diff(&base, &HashMap::new(), &new_persistent);
    assert!(diff.0.is_empty(), "No added entries");
    assert_eq!(diff.1.len(), 1, "Should detect 1 modified entry");
    assert_eq!(diff.1["CreatorBalance(GCREATOR,TOKEN)"], "200");
}

#[test]
fn test_incremental_diff_removed_entries() {
    let mut persistent = HashMap::new();
    persistent.insert("LockedTip(GCREATOR,1)".to_string(), "xdr_data".to_string());
    let base = make_state("CTEST", HashMap::new(), persistent);

    let diff = compute_diff(&base, &HashMap::new(), &HashMap::new());
    assert!(diff.0.is_empty());
    assert!(diff.1.is_empty());
    assert_eq!(diff.2.len(), 1, "Should detect 1 removed entry");
    assert_eq!(diff.2[0], "LockedTip(GCREATOR,1)");
}

#[test]
fn test_incremental_diff_no_changes() {
    let mut persistent = HashMap::new();
    persistent.insert("Admin".to_string(), "GADMIN".to_string());
    let base = make_state("CTEST", HashMap::new(), persistent.clone());

    let diff = compute_diff(&base, &HashMap::new(), &persistent);
    assert!(diff.0.is_empty(), "No added");
    assert!(diff.1.is_empty(), "No modified");
    assert!(diff.2.is_empty(), "No removed");
}

#[test]
fn test_backup_encryption_roundtrip() {
    let key: [u8; 32] = rand::random();
    let plaintext = b"sensitive contract state data";

    let encrypted = encrypt(plaintext, &key).expect("Encryption should succeed");
    assert_ne!(encrypted, plaintext, "Encrypted data should differ from plaintext");

    let decrypted = decrypt(&encrypted, &key).expect("Decryption should succeed");
    assert_eq!(decrypted, plaintext, "Decrypted data should match original");
}

#[test]
fn test_backup_decryption_wrong_key_fails() {
    let key: [u8; 32] = rand::random();
    let wrong_key: [u8; 32] = rand::random();
    let plaintext = b"contract state";

    let encrypted = encrypt(plaintext, &key).unwrap();
    let result = decrypt(&encrypted, &wrong_key);
    assert!(result.is_err(), "Decryption with wrong key should fail");
}

#[test]
fn test_backup_metadata_fields() {
    let state = make_state("CTEST", HashMap::new(), HashMap::new());
    assert_eq!(state.metadata.backup_type, BackupType::Full);
    assert!(state.metadata.base_backup_id.is_none());
    assert!(!state.metadata.encrypted);
    assert!(!state.metadata.checksum.is_empty());
    assert_eq!(state.metadata.tool_version, "0.1.0");
}

#[test]
fn test_recovery_verify_only_accepts_valid_backup() {
    // Simulates the --verify-only path of state-importer: load a backup from
    // disk, verify its checksum, and confirm it passes without actually restoring.
    let mut persistent = HashMap::new();
    persistent.insert("CreatorBalance(GCREATOR,TOKEN)".to_string(), "9999".to_string());
    persistent.insert("TipCounter".to_string(), "7".to_string());

    let state = make_state("CRECOVERY", HashMap::new(), persistent);

    // Write to a temp file (simulates what state-exporter produces).
    let tmp = std::env::temp_dir().join(format!("tipjar_test_{}.json", state.metadata.backup_id));
    let json = serde_json::to_string_pretty(&state).unwrap();
    std::fs::write(&tmp, &json).unwrap();

    // Read back and verify — mirrors what state-importer does before restoring.
    let loaded_json = std::fs::read_to_string(&tmp).unwrap();
    let loaded: ContractState = serde_json::from_str(&loaded_json).unwrap();

    assert!(verify_state_checksum(&loaded), "Loaded backup should pass integrity check");
    assert_eq!(loaded.contract_id, "CRECOVERY");
    assert_eq!(loaded.persistent_entries["TipCounter"], "7");

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn test_recovery_rejects_corrupted_backup() {
    let state = make_state("CCORRUPT", HashMap::new(), HashMap::new());
    let mut json: serde_json::Value = serde_json::to_value(&state).unwrap();

    // Corrupt a field after the checksum was embedded.
    json["ledger_sequence"] = serde_json::json!(99999);
    let corrupted: ContractState = serde_json::from_value(json).unwrap();

    assert!(!verify_state_checksum(&corrupted), "Corrupted backup should fail integrity check");
}

#[test]
fn test_incremental_backup_type_flag() {
    let base = make_state("CTEST", HashMap::new(), HashMap::new());
    let mut meta = base.metadata.clone();
    meta.backup_type = BackupType::Incremental;
    meta.base_backup_id = Some(base.metadata.backup_id.clone());

    assert_eq!(meta.backup_type, BackupType::Incremental);
    assert!(meta.base_backup_id.is_some());
}

// ── Inline crypto helpers (mirrors tools/backup/crypto.rs) ───────────────────

fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
    use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Key, Nonce};
    use base64::{engine::general_purpose::STANDARD as B64, Engine};

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce_bytes: [u8; 12] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext).map_err(|e| e.to_string())?;
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(B64.encode(&combined).into_bytes())
}

fn decrypt(encoded: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
    use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Key, Nonce};
    use base64::{engine::general_purpose::STANDARD as B64, Engine};

    let combined = B64.decode(encoded).map_err(|e| e.to_string())?;
    if combined.len() <= 12 { return Err("Ciphertext too short".into()); }
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher.decrypt(nonce, ciphertext).map_err(|e| e.to_string())
}

// ── Inline diff helper (mirrors tools/backup/state_exporter.rs) ───────────────

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
