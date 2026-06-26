//! Puzzle generation and difficulty calculation.

use soroban_sdk::{BytesN, Env};

use super::{difficulty_iterations, PuzzleDifficulty};

/// Generate a puzzle commitment from a secret and nonce.
///
/// Commitment = SHA-256(secret XOR nonce_bytes).
/// On-chain we use `env.crypto().sha256()` which is the only available hash.
pub fn generate_commitment(env: &Env, secret: &BytesN<32>, nonce: &BytesN<32>) -> BytesN<32> {
    // XOR secret and nonce to produce the pre-image.
    let secret_bytes = secret.to_array();
    let nonce_bytes = nonce.to_array();
    let mut pre_image = [0u8; 32];
    for i in 0..32 {
        pre_image[i] = secret_bytes[i] ^ nonce_bytes[i];
    }
    let pre_image_bytes = soroban_sdk::Bytes::from_array(env, &pre_image);
    BytesN::from_array(env, &env.crypto().sha256(&pre_image_bytes).to_array())
}

/// Calculate the number of sequential hash iterations for a given difficulty
/// and optional custom override.
///
/// If `custom_iterations` is `Some(n)` and `n > 0`, it is used directly.
/// Otherwise the difficulty tier's default is returned.
pub fn calculate_iterations(difficulty: PuzzleDifficulty, custom_iterations: Option<u64>) -> u64 {
    if let Some(n) = custom_iterations {
        if n > 0 {
            return n;
        }
    }
    difficulty_iterations(difficulty)
}

/// Verify that a proposed solution matches the stored commitment.
///
/// The solver must supply the original `secret` and `nonce`. We recompute the
/// commitment and compare it to the stored value.
pub fn verify_solution(
    env: &Env,
    commitment: &BytesN<32>,
    secret: &BytesN<32>,
    nonce: &BytesN<32>,
) -> bool {
    let recomputed = generate_commitment(env, secret, nonce);
    recomputed == *commitment
}

/// Check whether the unlock time has been reached.
pub fn is_time_reached(env: &Env, unlock_time: u64) -> bool {
    env.ledger().timestamp() >= unlock_time
}
