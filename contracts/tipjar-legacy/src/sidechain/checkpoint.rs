use soroban_sdk::{symbol_short, Address, BytesN, Env};

use crate::sidechain::{Checkpoint, TipBatch};
use crate::DataKey;

/// Submits a new checkpoint anchoring sidechain state on the main chain.
///
/// Only the registered sidechain operator may call this. Each checkpoint
/// increments the sequence number and records the sidechain state root.
pub fn submit_checkpoint(
    env: &Env,
    operator: &Address,
    state_root: BytesN<32>,
    tip_count: u32,
    total_volume: i128,
) -> u64 {
    operator.require_auth();

    let stored_op: Address = env
        .storage()
        .instance()
        .get(&DataKey::SidechainOperator)
        .expect("sidechain not initialized");
    if *operator != stored_op {
        panic!("unauthorized");
    }

    let seq: u64 = env
        .storage()
        .instance()
        .get(&DataKey::SidechainLatestCheckpoint)
        .unwrap_or(0)
        + 1;

    let checkpoint = Checkpoint {
        seq,
        state_root,
        tip_count,
        total_volume,
        submitted_at: env.ledger().timestamp(),
        finalized: false,
    };

    env.storage()
        .persistent()
        .set(&DataKey::SidechainCheckpoint(seq), &checkpoint);
    env.storage()
        .instance()
        .set(&DataKey::SidechainLatestCheckpoint, &seq);

    env.events().publish(
        (symbol_short!("sc_ckpt"), seq),
        (checkpoint.state_root.clone(), tip_count, total_volume),
    );

    seq
}

/// Finalizes a checkpoint, making it immutable and crediting batched tips.
///
/// After finalization the checkpoint cannot be overwritten. Any pending
/// `TipBatch` records linked to this checkpoint are settled into creator
/// balances.
pub fn finalize_checkpoint(env: &Env, operator: &Address, seq: u64) {
    operator.require_auth();

    let stored_op: Address = env
        .storage()
        .instance()
        .get(&DataKey::SidechainOperator)
        .expect("sidechain not initialized");
    if *operator != stored_op {
        panic!("unauthorized");
    }

    let mut checkpoint: Checkpoint = env
        .storage()
        .persistent()
        .get(&DataKey::SidechainCheckpoint(seq))
        .expect("checkpoint not found");

    if checkpoint.finalized {
        panic!("already finalized");
    }

    checkpoint.finalized = true;
    env.storage()
        .persistent()
        .set(&DataKey::SidechainCheckpoint(seq), &checkpoint);

    env.events()
        .publish((symbol_short!("sc_fin"), seq), checkpoint.total_volume);
}

/// Records a pending tip batch linked to a checkpoint.
pub fn record_tip_batch(
    env: &Env,
    creator: Address,
    token: Address,
    total_amount: i128,
    tip_count: u32,
    checkpoint_seq: u64,
) -> u64 {
    let batch_id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::SidechainBatchCounter)
        .unwrap_or(0)
        + 1;

    let batch = TipBatch {
        batch_id,
        creator,
        token,
        total_amount,
        tip_count,
        checkpoint_seq,
        finalized: false,
    };

    env.storage()
        .persistent()
        .set(&DataKey::SidechainBatch(batch_id), &batch);
    env.storage()
        .instance()
        .set(&DataKey::SidechainBatchCounter, &batch_id);

    batch_id
}

/// Settles a pending tip batch into the creator's on-chain balance.
///
/// Requires the linked checkpoint to be finalized first.
pub fn settle_batch(env: &Env, batch_id: u64) {
    let mut batch: TipBatch = env
        .storage()
        .persistent()
        .get(&DataKey::SidechainBatch(batch_id))
        .expect("batch not found");

    if batch.finalized {
        panic!("batch already settled");
    }

    let checkpoint: Checkpoint = env
        .storage()
        .persistent()
        .get(&DataKey::SidechainCheckpoint(batch.checkpoint_seq))
        .expect("checkpoint not found");

    if !checkpoint.finalized {
        panic!("checkpoint not finalized");
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

    // Update finalized total for this creator/token
    let fin_key = DataKey::SidechainFinalizedTotal(batch.creator.clone(), batch.token.clone());
    let fin: i128 = env.storage().persistent().get(&fin_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&fin_key, &(fin + batch.total_amount));

    batch.finalized = true;
    env.storage()
        .persistent()
        .set(&DataKey::SidechainBatch(batch_id), &batch);

    env.events().publish(
        (symbol_short!("sc_setl"), batch.creator.clone()),
        (batch.token.clone(), batch.total_amount, batch_id),
    );
}
