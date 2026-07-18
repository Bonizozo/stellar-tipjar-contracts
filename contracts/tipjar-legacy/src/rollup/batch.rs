use soroban_sdk::{symbol_short, Address, BytesN, Env};

use crate::rollup::{BatchStatus, RollupBatch, CHALLENGE_PERIOD};
use crate::DataKey;

/// Submits a new tip batch to the rollup.
///
/// The batch enters `Pending` status and is subject to the challenge period.
/// Returns the new batch ID.
pub fn submit_batch(
    env: &Env,
    sequencer: &Address,
    state_root: BytesN<32>,
    creator: Address,
    token: Address,
    total_amount: i128,
    tip_count: u32,
) -> u64 {
    sequencer.require_auth();

    let stored_seq: Address = env
        .storage()
        .instance()
        .get(&DataKey::RollupSequencer)
        .expect("rollup not initialized");
    if *sequencer != stored_seq {
        panic!("unauthorized");
    }

    let batch_id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::RollupBatchCounter)
        .unwrap_or(0)
        + 1;

    let batch = RollupBatch {
        batch_id,
        sequencer: sequencer.clone(),
        state_root,
        creator,
        token,
        total_amount,
        tip_count,
        submitted_at: env.ledger().timestamp(),
        status: BatchStatus::Pending,
    };

    env.storage()
        .persistent()
        .set(&DataKey::RollupBatch(batch_id), &batch);
    env.storage()
        .instance()
        .set(&DataKey::RollupBatchCounter, &batch_id);

    // Increment pending counter
    let pending: u64 = env
        .storage()
        .instance()
        .get(&DataKey::RollupPendingCount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::RollupPendingCount, &(pending + 1));

    env.events().publish(
        (symbol_short!("rl_sub"), batch_id),
        (batch.state_root.clone(), total_amount, tip_count),
    );

    batch_id
}

/// Finalizes a batch after the challenge period has elapsed with no valid fraud proof.
///
/// Credits the creator's balance. Anyone may call this.
pub fn finalize_batch(env: &Env, batch_id: u64) {
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
    if now < batch.submitted_at + challenge_period {
        panic!("challenge period not elapsed");
    }

    // Credit creator balance
    let bal_key = DataKey::CreatorBalance(batch.creator.clone(), batch.token.clone());
    let balance: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&bal_key, &(balance + batch.total_amount));

    let tot_key = DataKey::CreatorTotal(batch.creator.clone(), batch.token.clone());
    let total: i128 = env.storage().persistent().get(&tot_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&tot_key, &(total + batch.total_amount));

    batch.status = BatchStatus::Finalized;
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

    let finalized: u64 = env
        .storage()
        .instance()
        .get(&DataKey::RollupFinalizedCount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::RollupFinalizedCount, &(finalized + 1));

    env.events().publish(
        (symbol_short!("rl_fin"), batch_id),
        (
            batch.creator.clone(),
            batch.token.clone(),
            batch.total_amount,
        ),
    );
}
