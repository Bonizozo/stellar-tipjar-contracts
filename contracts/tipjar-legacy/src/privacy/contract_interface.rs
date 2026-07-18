//! Contract interface for homomorphic encryption operations.
//!
//! Exposes public contract functions for:
//! - Initializing homomorphic encryption
//! - Creating encrypted tips
//! - Managing encrypted balances
//! - Rotating encryption keys
//! - Revealing encrypted amounts

use soroban_sdk::{Address, BytesN, Env, Symbol, Vec};

use super::encrypted_operations::{
    aggregate_encrypted_tips, compute_encrypted_fee, create_encrypted_tip, get_encrypted_balance,
    reveal_encrypted_tip,
};
use super::homomorphic::{EncryptedAmount, HomomorphicPublicKey, RangeProof};
use super::key_management::{
    get_current_public_key, initialize_homomorphic, is_homomorphic_enabled, rotate_key,
    KeyManagementConfig,
};

/// Initialize homomorphic encryption for the contract.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `admin` - Admin address (must be authorized)
/// * `n` - Modulus for public key
/// * `g` - Generator for public key
/// * `key_version` - Initial key version
/// * `rotation_interval` - Key rotation interval in seconds
/// * `max_key_age` - Maximum key age before forced rotation
///
/// # Returns
/// Ok(()) on success, Err on failure
pub fn init_homomorphic(
    env: Env,
    admin: Address,
    n: BytesN<32>,
    g: BytesN<32>,
    key_version: u32,
    rotation_interval: u64,
    max_key_age: u64,
) -> Result<(), Symbol> {
    let public_key = HomomorphicPublicKey {
        n,
        g,
        version: key_version,
    };

    let config = KeyManagementConfig {
        rotation_interval,
        max_key_age,
        key_history_size: 10,
        rotation_requires_approval: true,
    };

    initialize_homomorphic(&env, &admin, public_key, config).map_err(|e| Symbol::new(&env, e))
}

/// Create an encrypted tip.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `sender` - Tipper address (must be authorized)
/// * `creator` - Creator address
/// * `token` - Token address
/// * `amount` - Plaintext tip amount
/// * `randomness_seed` - Seed for encryption randomness
/// * `range_proof` - Proof that amount is in valid range
///
/// # Returns
/// Encrypted tip ID
pub fn tip_encrypted(
    env: Env,
    sender: Address,
    creator: Address,
    token: Address,
    amount: i128,
    randomness_seed: BytesN<32>,
    range_proof: RangeProof,
) -> Result<u64, Symbol> {
    sender.require_auth();

    create_encrypted_tip(
        &env,
        &sender,
        &creator,
        &token,
        amount,
        &randomness_seed,
        &range_proof,
    )
    .map_err(|e| Symbol::new(&env, e))
}

/// Get encrypted balance for a creator.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `creator` - Creator address
/// * `token` - Token address
///
/// # Returns
/// Encrypted balance record
pub fn get_encrypted_balance_for(
    env: Env,
    creator: Address,
    token: Address,
) -> Result<EncryptedAmount, Symbol> {
    get_encrypted_balance(&env, &creator, &token)
        .map(|balance| balance.encrypted_amount)
        .map_err(|e| Symbol::new(&env, e))
}

/// Reveal an encrypted tip amount.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `tip_id` - Encrypted tip ID
/// * `decrypted_amount` - Plaintext amount
/// * `creator` - Creator address (must be authorized)
///
/// # Returns
/// Ok(()) on success, Err on failure
pub fn reveal_encrypted_tip_amount(
    env: Env,
    tip_id: u64,
    decrypted_amount: i128,
    creator: Address,
) -> Result<(), Symbol> {
    reveal_encrypted_tip(&env, tip_id, decrypted_amount, &creator).map_err(|e| Symbol::new(&env, e))
}

/// Rotate to a new encryption key.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `admin` - Admin address (must be authorized)
/// * `new_n` - New modulus
/// * `new_g` - New generator
/// * `new_version` - New key version
/// * `reason` - Reason for rotation
///
/// # Returns
/// New key version
pub fn rotate_encryption_key(
    env: Env,
    admin: Address,
    new_n: BytesN<32>,
    new_g: BytesN<32>,
    new_version: u32,
    reason: Symbol,
) -> Result<u32, Symbol> {
    let new_key = HomomorphicPublicKey {
        n: new_n,
        g: new_g,
        version: new_version,
    };

    rotate_key(&env, &admin, new_key, reason).map_err(|e| Symbol::new(&env, e))
}

/// Get current public key.
///
/// # Arguments
/// * `env` - Soroban environment
///
/// # Returns
/// Current public key
pub fn get_public_key(env: Env) -> Result<HomomorphicPublicKey, Symbol> {
    get_current_public_key(&env).map_err(|e| Symbol::new(&env, e))
}

/// Check if homomorphic encryption is enabled.
///
/// # Arguments
/// * `env` - Soroban environment
///
/// # Returns
/// true if enabled, false otherwise
pub fn is_encrypted_tips_enabled(env: Env) -> bool {
    is_homomorphic_enabled(&env)
}

/// Aggregate multiple encrypted tips.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `encrypted_tips` - Vector of encrypted amounts
///
/// # Returns
/// Aggregated encrypted amount
pub fn aggregate_tips_encrypted(
    env: Env,
    encrypted_tips: Vec<EncryptedAmount>,
) -> Result<EncryptedAmount, Symbol> {
    aggregate_encrypted_tips(&env, &encrypted_tips).map_err(|e| Symbol::new(&env, e))
}

/// Compute encrypted fee on encrypted amount.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `encrypted_amount` - Encrypted tip amount
/// * `fee_basis_points` - Fee in basis points
///
/// # Returns
/// Encrypted fee amount
pub fn compute_fee_encrypted(
    env: Env,
    encrypted_amount: EncryptedAmount,
    fee_basis_points: u32,
) -> Result<EncryptedAmount, Symbol> {
    compute_encrypted_fee(&encrypted_amount, fee_basis_points).map_err(|e| Symbol::new(&env, e))
}
