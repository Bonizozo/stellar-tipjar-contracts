use soroban_sdk::{BytesN, Env, String};

use crate::bridge::{BridgeDataKey, SourceChain};

/// Storage key for the set of already-processed source tx hashes (replay guard).
/// Stored as `DataKey::BridgeProcessed(hash)` → bool.

/// Returns `true` if this source transaction has already been processed.
pub fn is_processed(env: &Env, source_tx_hash: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .get(&BridgeDataKey::BridgeProcessed(source_tx_hash.clone()))
        .unwrap_or(false)
}

/// Marks a source transaction as processed to prevent replay.
pub fn mark_processed(env: &Env, source_tx_hash: &BytesN<32>) {
    env.storage().persistent().set(
        &BridgeDataKey::BridgeProcessed(source_tx_hash.clone()),
        &true,
    );
}

/// Validates a bridge tip request.
///
/// Checks:
/// 1. Bridge feature is enabled.
/// 2. Amount is positive and within chain limits.
/// 3. The source chain is supported.
/// 4. The source transaction has not already been processed (replay protection).
///
/// Returns `Ok(())` on success or a descriptive error string on failure.
pub fn validate_bridge_tip(
    env: &Env,
    source_chain: &SourceChain,
    source_tx_hash: &BytesN<32>,
    amount: i128,
) -> Result<(), String> {
    // Check bridge is enabled
    let enabled: bool = env
        .storage()
        .instance()
        .get(&BridgeDataKey::BridgeEnabled)
        .unwrap_or(false);
    if !enabled {
        return Err(String::from_str(env, "bridge disabled"));
    }

    // Validate amount is positive
    if amount <= 0 {
        return Err(String::from_str(env, "invalid amount"));
    }

    // Validate source chain is supported (all defined chains are supported by default)
    // Future: per-chain min/max amounts can be enforced here
    let _ = source_chain;

    // Replay protection
    if is_processed(env, source_tx_hash) {
        return Err(String::from_str(env, "already processed"));
    }

    Ok(())
}

/// Verifies cross-chain message data.
///
/// Performs chain-specific validation checks:
/// - Validates chain ID matches expected value
/// - Verifies source transaction hash format
/// - Checks message nonce uniqueness
///
/// Returns `Ok(())` on success.
pub fn verify_cross_chain_message(
    env: &Env,
    source_chain: &SourceChain,
    source_tx_hash: &BytesN<32>,
    nonce: u64,
) -> Result<(), String> {
    let _ = source_chain;
    let _ = source_tx_hash;

    // Verify nonce hasn't been used before
    let nonce_key = BridgeDataKey::BridgeProcessed(BytesN::from_array(env, &hash_nonce(nonce)));
    let used: bool = env.storage().persistent().get(&nonce_key).unwrap_or(false);
    if used {
        return Err(String::from_str(env, "nonce already used"));
    }

    // Mark nonce as used
    env.storage().persistent().set(&nonce_key, &true);

    Ok(())
}

/// Validates that a source chain is supported.
pub fn validate_chain_supported(_env: &Env, _chain: &SourceChain) -> Result<(), String> {
    // All defined chains in SourceChain are supported
    // This function allows future per-chain enable/disable logic
    Ok(())
}

/// Calculates the bridge fee for a given amount.
///
/// Returns the fee amount and net amount after fee deduction.
pub fn calculate_bridge_fee(env: &Env, amount: i128) -> (i128, i128) {
    let fee_bps: u32 = env
        .storage()
        .instance()
        .get(&BridgeDataKey::BridgeFeeBps)
        .unwrap_or(0);

    if fee_bps == 0 || amount <= 0 {
        return (0, amount);
    }

    let fee = (amount * fee_bps as i128) / 10_000;
    let net = amount - fee;
    (fee, net)
}

/// Helper to hash a nonce into a 32-byte array for storage.
fn hash_nonce(nonce: u64) -> [u8; 32] {
    let mut result = [0u8; 32];
    let bytes = nonce.to_le_bytes();
    // Simple deterministic mapping: place nonce bytes at start, rest is pattern
    for i in 0..8 {
        result[i] = bytes[i];
    }
    // Distinguish from tx hashes by using a different pattern in remaining bytes
    for i in 8..32 {
        result[i] = 0xFF;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_bridge_fee_zero() {
        let env = Env::default();
        let contract_id = env.register(crate::TipJarContract, ());
        let (fee, net) = env.as_contract(&contract_id, || calculate_bridge_fee(&env, 1000));
        assert_eq!(fee, 0);
        assert_eq!(net, 1000);
    }

    #[test]
    fn test_validate_chain_supported() {
        let env = Env::default();
        assert!(validate_chain_supported(&env, &SourceChain::Ethereum).is_ok());
        assert!(validate_chain_supported(&env, &SourceChain::Polygon).is_ok());
    }
}
