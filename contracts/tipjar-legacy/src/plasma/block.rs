use soroban_sdk::{symbol_short, Address, BytesN, Env};

use crate::plasma::{PlasmaBlock, PlasmaBlockStatus, PlasmaKey, MAX_TIPS_PER_BLOCK};
use crate::DataKey;

/// Commits a new Plasma block to the main chain.
///
/// Only the authorized operator may call this. The block enters `Committed`
/// status and must be finalized after a delay to allow for challenges.
/// Returns the new block number.
pub fn commit_block(
    env: &Env,
    operator: &Address,
    tx_root: BytesN<32>,
    total_volume: i128,
    tip_count: u32,
) -> u64 {
    operator.require_auth();

    let stored_op: Address = env
        .storage()
        .instance()
        .get(&DataKey::PlasmaOperator)
        .expect("Plasma not initialized");
    if *operator != stored_op {
        panic!("unauthorized");
    }

    if tip_count > MAX_TIPS_PER_BLOCK {
        panic!("tip count exceeds maximum");
    }

    let block_number: u64 = env
        .storage()
        .instance()
        .get(&DataKey::PlasmaLatestBlock)
        .unwrap_or(0)
        + 1;

    let block = PlasmaBlock {
        block_number,
        tx_root,
        operator: operator.clone(),
        total_volume,
        tip_count,
        committed_at: env.ledger().timestamp(),
        status: PlasmaBlockStatus::Committed,
    };

    env.storage()
        .persistent()
        .set(&DataKey::PlasmaBlock(block_number), &block);
    env.storage()
        .instance()
        .set(&DataKey::PlasmaLatestBlock, &block_number);

    let total_blocks: u64 = env
        .storage()
        .instance()
        .get(&DataKey::PlasmaBlockCounter)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::PlasmaBlockCounter, &(total_blocks + 1));

    env.events().publish(
        (symbol_short!("pl_block"), block_number),
        (tx_root, total_volume, tip_count),
    );

    block_number
}

/// Finalizes a committed Plasma block, making exits from it valid.
///
/// Anyone may call this after a sufficient delay to allow for challenges.
pub fn finalize_block(env: &Env, block_number: u64) {
    let mut block: PlasmaBlock = env
        .storage()
        .persistent()
        .get(&DataKey::PlasmaBlock(block_number))
        .expect("block not found");

    if !matches!(block.status, PlasmaBlockStatus::Committed) {
        panic!("block not committed");
    }

    // Require a minimum delay before finalization (e.g., 1 hour)
    const MIN_FINALIZATION_DELAY: u64 = 3600;
    let now = env.ledger().timestamp();
    if now < block.committed_at + MIN_FINALIZATION_DELAY {
        panic!("finalization delay not elapsed");
    }

    block.status = PlasmaBlockStatus::Finalized;
    env.storage()
        .persistent()
        .set(&DataKey::PlasmaBlock(block_number), &block);

    env.events()
        .publish((symbol_short!("pl_final"), block_number), block.tx_root);
}

/// Invalidates a Plasma block after a successful challenge.
///
/// This is called internally by the challenge module when a block is proven invalid.
pub fn invalidate_block(env: &Env, block_number: u64) {
    let mut block: PlasmaBlock = env
        .storage()
        .persistent()
        .get(&DataKey::PlasmaBlock(block_number))
        .expect("block not found");

    block.status = PlasmaBlockStatus::Invalidated;
    env.storage()
        .persistent()
        .set(&DataKey::PlasmaBlock(block_number), &block);

    env.events().publish(
        (symbol_short!("pl_inval"), block_number),
        block.tx_root,
    );
}

/// Returns a Plasma block by block number.
pub fn get_block(env: &Env, block_number: u64) -> Option<PlasmaBlock> {
    env.storage()
        .persistent()
        .get(&DataKey::PlasmaBlock(block_number))
}

/// Returns the latest committed block number.
pub fn get_latest_block_number(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::PlasmaLatestBlock)
        .unwrap_or(0)
}
