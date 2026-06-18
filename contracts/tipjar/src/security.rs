/// Security utilities and guards for the TipJar contract.
use soroban_sdk::{Env, Symbol};

/// Reentrancy guard key
pub const REENTRANCY_LOCK_KEY: &str = "reentrancy_lock";

/// Maximum tip amount to prevent overflow (1 billion tokens with 7 decimals)
pub const MAX_TIP_AMOUNT: i128 = 10_000_000_000_000_000;

/// Maximum batch size for operations
pub const MAX_BATCH_SIZE: u32 = 100;

/// Reentrancy guard data key
#[soroban_sdk::contracttype]
#[derive(Clone)]
pub enum SecurityKey {
    ReentrancyLock,
}

/// Set reentrancy lock
pub fn set_lock(env: &Env) {
    env.storage()
        .instance()
        .set(&SecurityKey::ReentrancyLock, &true);
}

/// Release reentrancy lock
pub fn release_lock(env: &Env) {
    env.storage()
        .instance()
        .set(&SecurityKey::ReentrancyLock, &false);
}

/// Check if reentrancy lock is active
pub fn is_locked(env: &Env) -> bool {
    env.storage()
        .instance()
        .get::<_, bool>(&SecurityKey::ReentrancyLock)
        .unwrap_or(false)
}

/// Validate tip amount
pub fn validate_amount(amount: i128) -> Result<(), u32> {
    if amount <= 0 {
        return Err(3); // InvalidAmount
    }
    if amount > MAX_TIP_AMOUNT {
        return Err(3); // InvalidAmount
    }
    Ok(())
}

/// Validate batch size
pub fn validate_batch_size(size: u32) -> Result<(), u32> {
    if size == 0 || size > MAX_BATCH_SIZE {
        return Err(10); // BatchTooLarge
    }
    Ok(())
}
