use soroban_sdk::{BytesN, Env};

use crate::privacy::{commitment, CommitmentOpening, PrivateTip};
use crate::DataKey;

/// Verifies a private tip: checks the nullifier hasn't been used and the commitment opens correctly.
///
/// Returns `Ok(())` if valid, `Err(msg)` otherwise.
pub fn verify_private_tip(
    env: &Env,
    tip: &PrivateTip,
    opening: &CommitmentOpening,
) -> Result<(), &'static str> {
    // 1. Check nullifier hasn't been used (prevents double-spend).
    if is_nullifier_used(env, &tip.nullifier) {
        return Err("nullifier already used");
    }

    // 2. Verify commitment opening.
    if !commitment::verify_opening(env, &tip.commitment, opening) {
        return Err("invalid commitment opening");
    }

    Ok(())
}

/// Marks a nullifier as used.
pub fn mark_nullifier_used(env: &Env, nullifier: &BytesN<32>) {
    env.storage()
        .persistent()
        .set(&DataKey::PrivacyNullifier(nullifier.clone()), &true);
}

/// Returns true if the nullifier has already been used.
pub fn is_nullifier_used(env: &Env, nullifier: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::PrivacyNullifier(nullifier.clone()))
        .unwrap_or(false)
}
