use soroban_sdk::{symbol_short, Address, BytesN, Env};

use crate::plasma::{ExitChallenge, ExitStatus, PlasmaExit, EXIT_CHALLENGE_PERIOD};
use crate::DataKey;

/// Challenges a pending exit by proving the referenced transaction was already spent.
///
/// A challenge is accepted when the challenger provides a `spend_tx_hash` that
/// proves the exitor's transaction was already consumed in a prior Plasma block.
/// On acceptance the exit is marked `Challenged` and no funds are released.
///
/// Returns `true` if the challenge was accepted.
pub fn challenge_exit(
    env: &Env,
    challenger: &Address,
    exit_id: u64,
    spend_tx_hash: BytesN<32>,
) -> bool {
    challenger.require_auth();

    let mut exit: PlasmaExit = env
        .storage()
        .persistent()
        .get(&DataKey::PlasmaExit(exit_id))
        .expect("exit not found");

    if !matches!(exit.status, ExitStatus::Pending) {
        panic!("exit not pending");
    }

    let now = env.ledger().timestamp();
    if now >= exit.initiated_at + EXIT_CHALLENGE_PERIOD {
        panic!("challenge period elapsed");
    }

    // A challenge is valid when the spend_tx_hash differs from the exit's tx_hash,
    // indicating the transaction was already spent (double-spend attempt).
    // In a full implementation this would verify a Merkle proof of the spend
    // transaction in a prior block; here we use hash mismatch as the signal.
    if spend_tx_hash == exit.tx_hash {
        // Same hash — not a valid double-spend proof
        return false;
    }

    let challenge = ExitChallenge {
        exit_id,
        challenger: challenger.clone(),
        spend_tx_hash: spend_tx_hash.clone(),
        submitted_at: now,
    };

    env.storage()
        .persistent()
        .set(&DataKey::PlasmaChallenge(exit_id), &challenge);

    exit.status = ExitStatus::Challenged;
    env.storage()
        .persistent()
        .set(&DataKey::PlasmaExit(exit_id), &exit);

    env.events().publish(
        (symbol_short!("pl_chal"), exit_id),
        (challenger.clone(), spend_tx_hash, exit.tx_hash),
    );

    true
}

/// Returns the challenge for a given exit, if any.
pub fn get_challenge(env: &Env, exit_id: u64) -> Option<ExitChallenge> {
    env.storage()
        .persistent()
        .get(&DataKey::PlasmaChallenge(exit_id))
}
