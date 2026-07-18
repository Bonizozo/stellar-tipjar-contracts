use soroban_sdk::{symbol_short, Address, BytesN, Env};

use crate::rollup::{BatchStatus, FraudProof, RollupBatch, CHALLENGE_PERIOD};
use crate::DataKey;

/// Submits a fraud proof challenging a pending batch.
///
/// A fraud proof is accepted when the challenger provides a `claimed_root`
/// that differs from the batch's `state_root`, within the challenge period.
/// On acceptance the batch is marked `Challenged` and no credits are applied.
///
/// The challenger must be a registered verifier or any address (permissionless
/// challenge model). Returns `true` if the challenge was accepted.
pub fn submit_fraud_proof(
    env: &Env,
    challenger: &Address,
    batch_id: u64,
    claimed_root: BytesN<32>,
) -> bool {
    challenger.require_auth();

    let mut batch: RollupBatch = env
        .storage()
        .persistent()
        .get(&DataKey::RollupBatch(batch_id))
        .expect("batch not found");

    if !matches!(batch.status, BatchStatus::Pending) {
        panic!("batch not pending");
    }

    let challenge_period: u64 = env
        .storage()
        .instance()
        .get(&DataKey::RollupChallengePeriod)
        .unwrap_or(CHALLENGE_PERIOD);

    let now = env.ledger().timestamp();
    if now >= batch.submitted_at + challenge_period {
        panic!("challenge period elapsed");
    }

    // Fraud proof is valid when the claimed root differs from the submitted root.
    // In a full implementation this would verify a Merkle/execution proof;
    // here we use root mismatch as the on-chain verifiable signal.
    if claimed_root == batch.state_root {
        // Roots match — no fraud detected
        return false;
    }

    let proof = FraudProof {
        batch_id,
        challenger: challenger.clone(),
        claimed_root: claimed_root.clone(),
        submitted_at: now,
    };

    env.storage()
        .persistent()
        .set(&DataKey::RollupFraudProof(batch_id), &proof);

    batch.status = BatchStatus::Challenged;
    env.storage()
        .persistent()
        .set(&DataKey::RollupBatch(batch_id), &batch);

    // Update counters
    let pending: u64 = env
        .storage()
        .instance()
        .get(&DataKey::RollupPendingCount)
        .unwrap_or(1);
    env.storage()
        .instance()
        .set(&DataKey::RollupPendingCount, &pending.saturating_sub(1));

    let challenged: u64 = env
        .storage()
        .instance()
        .get(&DataKey::RollupChallengedCount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::RollupChallengedCount, &(challenged + 1));

    env.events().publish(
        (symbol_short!("rl_fraud"), batch_id),
        (challenger.clone(), claimed_root, batch.state_root),
    );

    true
}

/// Returns the fraud proof for a challenged batch, if any.
pub fn get_fraud_proof(env: &Env, batch_id: u64) -> Option<FraudProof> {
    env.storage()
        .persistent()
        .get(&DataKey::RollupFraudProof(batch_id))
}
