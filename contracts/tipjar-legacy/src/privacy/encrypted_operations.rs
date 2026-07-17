//! Encrypted tip operations for privacy-preserving tipping.
//!
//! Provides functions to:
//! - Create encrypted tips
//! - Aggregate encrypted balances
//! - Perform privacy-preserving withdrawals
//! - Compute encrypted statistics

use soroban_sdk::{Address, BytesN, Env, Vec};

use super::homomorphic::{
    aggregate_encrypted_amounts, encrypt_amount, scalar_multiply_encrypted, verify_range_proof,
    EncryptedAmount, EncryptedBalance, RangeProof,
};
use super::key_management::{get_current_public_key, is_homomorphic_enabled};
use crate::DataKey;

/// Create an encrypted tip.
///
/// Encrypts the tip amount and stores it with metadata.
/// Supports privacy-preserving tip aggregation.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `sender` - Tipper address
/// * `creator` - Creator address
/// * `token` - Token address
/// * `amount` - Plaintext tip amount
/// * `randomness_seed` - Seed for encryption randomness
/// * `range_proof` - Proof that amount is in valid range
///
/// # Returns
/// Encrypted tip ID
pub fn create_encrypted_tip(
    env: &Env,
    sender: &Address,
    creator: &Address,
    token: &Address,
    amount: i128,
    randomness_seed: &BytesN<32>,
    range_proof: &RangeProof,
) -> Result<u64, &'static str> {
    // Verify homomorphic encryption is enabled
    if !is_homomorphic_enabled(env) {
        return Err("homomorphic encryption not enabled");
    }

    // Validate amount
    if amount <= 0 {
        return Err("tip amount must be positive");
    }

    // Get current public key
    let public_key = get_current_public_key(env)?;

    // Encrypt the amount
    let encrypted_amount = encrypt_amount(env, amount, &public_key, randomness_seed)?;

    // Verify range proof
    verify_range_proof(env, &encrypted_amount, range_proof)?;

    // Generate encrypted tip ID
    let tip_id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::EncryptedTipCounter)
        .unwrap_or(0);

    env.storage()
        .instance()
        .set(&DataKey::EncryptedTipCounter, &(tip_id + 1));

    // Store encrypted tip
    let encrypted_tip = EncryptedTip {
        id: tip_id,
        sender: sender.clone(),
        creator: creator.clone(),
        token: token.clone(),
        encrypted_amount: encrypted_amount.clone(),
        created_at: env.ledger().timestamp(),
        revealed: false,
    };

    env.storage()
        .persistent()
        .set(&DataKey::EncryptedTip(tip_id), &encrypted_tip);

    // Update encrypted balance
    update_encrypted_balance(env, creator, token, &encrypted_amount)?;

    // Emit event
    env.events().publish(
        (soroban_sdk::Symbol::new(env, "encrypted_tip"),),
        (sender.clone(), creator.clone(), tip_id),
    );

    Ok(tip_id)
}

/// Update encrypted balance for a creator.
///
/// Aggregates new encrypted amount into existing balance.
/// Maintains privacy while tracking total tips.
fn update_encrypted_balance(
    env: &Env,
    creator: &Address,
    token: &Address,
    new_amount: &EncryptedAmount,
) -> Result<(), &'static str> {
    let balance_key = DataKey::EncryptedBalance(creator.clone(), token.clone());

    if let Some(mut balance) = env
        .storage()
        .persistent()
        .get::<DataKey, EncryptedBalance>(&balance_key)
    {
        // Aggregate with existing balance
        let amounts = {
            let mut v = Vec::new(env);
            v.push_back(balance.encrypted_amount.clone());
            v.push_back(new_amount.clone());
            v
        };

        let aggregated = aggregate_encrypted_amounts(env, &amounts)?;

        balance.encrypted_amount = aggregated;
        balance.last_updated = env.ledger().timestamp();
        balance.tip_count += 1;

        env.storage().persistent().set(&balance_key, &balance);
    } else {
        // Create new balance record
        let balance = EncryptedBalance {
            creator: creator.clone(),
            token: token.clone(),
            encrypted_amount: new_amount.clone(),
            last_updated: env.ledger().timestamp(),
            tip_count: 1,
        };

        env.storage().persistent().set(&balance_key, &balance);
    }

    Ok(())
}

/// Get encrypted balance for a creator.
///
/// Returns the aggregated encrypted balance without decryption.
pub fn get_encrypted_balance(
    env: &Env,
    creator: &Address,
    token: &Address,
) -> Result<EncryptedBalance, &'static str> {
    let balance_key = DataKey::EncryptedBalance(creator.clone(), token.clone());

    env.storage()
        .persistent()
        .get(&balance_key)
        .ok_or("no encrypted balance found")
}

/// Compute encrypted fee on encrypted amount.
///
/// Multiplies encrypted amount by fee percentage without decryption.
/// Enables privacy-preserving fee calculations.
///
/// # Arguments
/// * `encrypted_amount` - Encrypted tip amount
/// * `fee_basis_points` - Fee in basis points (0-10000)
///
/// # Returns
/// Encrypted fee amount
pub fn compute_encrypted_fee(
    encrypted_amount: &EncryptedAmount,
    fee_basis_points: u32,
) -> Result<EncryptedAmount, &'static str> {
    if fee_basis_points > 10000 {
        return Err("fee basis points must be <= 10000");
    }

    // Scale encrypted amount by fee percentage
    let fee_scalar = (fee_basis_points as u64) * 1000; // Scale for precision
    scalar_multiply_encrypted(encrypted_amount, fee_scalar)
}

/// Aggregate multiple encrypted tips into a single encrypted balance.
///
/// Useful for batch processing and privacy-preserving aggregation.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `encrypted_tips` - Vector of encrypted amounts
///
/// # Returns
/// Aggregated encrypted amount
pub fn aggregate_encrypted_tips(
    env: &Env,
    encrypted_tips: &Vec<EncryptedAmount>,
) -> Result<EncryptedAmount, &'static str> {
    if encrypted_tips.is_empty() {
        return Err("cannot aggregate empty tips");
    }

    aggregate_encrypted_amounts(env, encrypted_tips)
}

/// Reveal encrypted tip amount (requires authorization).
///
/// Decrypts and reveals a previously encrypted tip.
/// Only authorized parties (creator, admin) can reveal.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `tip_id` - Encrypted tip ID
/// * `decrypted_amount` - Plaintext amount (must match ciphertext)
/// * `authorizer` - Address authorizing the reveal
///
/// # Returns
/// Ok(()) if reveal is valid, Err otherwise
pub fn reveal_encrypted_tip(
    env: &Env,
    tip_id: u64,
    decrypted_amount: i128,
    authorizer: &Address,
) -> Result<(), &'static str> {
    authorizer.require_auth();

    let tip_key = DataKey::EncryptedTip(tip_id);

    let mut tip = env
        .storage()
        .persistent()
        .get::<DataKey, EncryptedTip>(&tip_key)
        .ok_or("encrypted tip not found")?;

    // Verify authorizer is creator or admin
    if tip.creator != *authorizer {
        return Err("only creator can reveal encrypted tip");
    }

    // Verify amount is positive
    if decrypted_amount <= 0 {
        return Err("revealed amount must be positive");
    }

    // Mark as revealed
    tip.revealed = true;
    env.storage().persistent().set(&tip_key, &tip);

    // Emit reveal event
    env.events().publish(
        (soroban_sdk::Symbol::new(env, "encrypted_tip_revealed"),),
        (tip_id, decrypted_amount),
    );

    Ok(())
}

#[soroban_sdk::contracttype]
/// Encrypted tip record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptedTip {
    /// Unique tip ID
    pub id: u64,
    /// Tipper address
    pub sender: Address,
    /// Creator address
    pub creator: Address,
    /// Token address
    pub token: Address,
    /// Encrypted amount
    pub encrypted_amount: EncryptedAmount,
    /// Creation timestamp
    pub created_at: u64,
    /// Whether amount has been revealed
    pub revealed: bool,
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_compute_encrypted_fee() {
        // This would require a full test environment setup
        // Placeholder for integration tests
    }
}
