//! Tip queuing logic.
//!
//! Tippers call `queue_tip` to add a pending tip to the open aggregation batch
//! for a `(token, creator)` pair. Tokens are transferred into the contract
//! immediately and held in escrow until the batch is settled or cancelled.

use soroban_sdk::{panic_with_error, token, Address, Env, Vec};

use crate::{DataKey, TipJarError};

use super::{batch, QueuedTip};

// ── Public API ───────────────────────────────────────────────────────────────

/// Queues a tip for aggregation.
///
/// Transfers `amount` of `token` from `tipper` into the contract immediately.
/// The tip is added to the open batch for `(token, creator)`, creating a new
/// batch if none exists.
///
/// Returns `(batch_id, tip_index)`.
///
/// Panics if:
/// - Aggregation is disabled.
/// - `amount` is not positive.
/// - `token` is not whitelisted.
/// - The open batch is already full (`max_batch_size`).
pub fn queue_tip(
    env: &Env,
    tipper: &Address,
    creator: &Address,
    token: &Address,
    amount: i128,
) -> (u64, u32) {
    tipper.require_auth();

    if amount <= 0 {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    // Token must be whitelisted
    let whitelisted: bool = env
        .storage()
        .instance()
        .get(&DataKey::TokenWhitelist(token.clone()))
        .unwrap_or(false);
    if !whitelisted {
        panic_with_error!(env, TipJarError::TokenNotWhitelisted);
    }

    let config = batch::get_config(env);
    if !config.enabled {
        panic_with_error!(env, TipJarError::AggregationDisabled);
    }

    // Get or open a batch for this (token, creator) pair
    let mut current_batch = batch::get_open_batch(env, token, creator)
        .unwrap_or_else(|| batch::open_batch(env, token, creator));

    if current_batch.tip_count >= config.max_batch_size {
        panic_with_error!(env, TipJarError::AggregationBatchFull);
    }

    // Transfer tokens from tipper into contract escrow immediately
    token::Client::new(env, token).transfer(tipper, &env.current_contract_address(), &amount);

    let tip_index = current_batch.tip_count;
    let batch_id = current_batch.id;

    // Record the queued tip
    let queued = QueuedTip {
        batch_id,
        index: tip_index,
        tipper: tipper.clone(),
        amount,
        queued_at: env.ledger().timestamp(),
        refunded: false,
    };
    env.storage()
        .persistent()
        .set(&DataKey::AggregationQueuedTip(batch_id, tip_index), &queued);

    // Track this tipper's queued tip indices for this batch (for cancellation lookup)
    track_tipper_tip(env, batch_id, tipper, tip_index);

    // Update batch totals
    current_batch.total_queued = current_batch
        .total_queued
        .checked_add(amount)
        .expect("queued overflow");
    current_batch.tip_count += 1;
    batch::save_batch(env, &current_batch);

    env.events().publish(
        (soroban_sdk::symbol_short!("agg_q"),),
        (tipper.clone(), creator.clone(), token.clone(), amount, batch_id, tip_index),
    );

    (batch_id, tip_index)
}

/// Cancels a queued tip and refunds the tipper.
///
/// Only the original tipper may cancel their own tip.
/// Cannot cancel after the batch has been settled.
///
/// Panics if:
/// - Tip not found.
/// - Caller is not the original tipper.
/// - Batch is already settled.
/// - Tip was already refunded.
pub fn cancel_queued_tip(
    env: &Env,
    tipper: &Address,
    batch_id: u64,
    tip_index: u32,
) {
    tipper.require_auth();

    let mut queued: QueuedTip = env
        .storage()
        .persistent()
        .get(&DataKey::AggregationQueuedTip(batch_id, tip_index))
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AggregationTipNotFound));

    if queued.tipper != *tipper {
        panic_with_error!(env, TipJarError::Unauthorized);
    }

    if queued.refunded {
        panic_with_error!(env, TipJarError::AggregationTipAlreadyRefunded);
    }

    let current_batch: super::AggregationBatch = env
        .storage()
        .persistent()
        .get(&DataKey::AggregationBatch(batch_id))
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AggregationBatchNotFound));

    if current_batch.status == super::BatchStatus::Settled {
        panic_with_error!(env, TipJarError::AggregationBatchAlreadySettled);
    }

    // Mark as refunded
    queued.refunded = true;
    env.storage()
        .persistent()
        .set(&DataKey::AggregationQueuedTip(batch_id, tip_index), &queued);

    // Reduce batch total (but keep tip_count so indices stay stable)
    let mut updated_batch = current_batch;
    updated_batch.total_queued = updated_batch
        .total_queued
        .saturating_sub(queued.amount);
    batch::save_batch(env, &updated_batch);

    // Refund tokens to tipper
    let token_client = token::Client::new(env, &updated_batch.token);
    token_client.transfer(&env.current_contract_address(), tipper, &queued.amount);

    env.events().publish(
        (soroban_sdk::symbol_short!("agg_cncl"),),
        (tipper.clone(), batch_id, tip_index, queued.amount),
    );
}

/// Returns a queued tip record, or `None`.
pub fn get_queued_tip(env: &Env, batch_id: u64, tip_index: u32) -> Option<QueuedTip> {
    env.storage()
        .persistent()
        .get(&DataKey::AggregationQueuedTip(batch_id, tip_index))
}

/// Returns all tip indices queued by `tipper` in `batch_id`.
pub fn get_tipper_tips(env: &Env, batch_id: u64, tipper: &Address) -> Vec<u32> {
    env.storage()
        .persistent()
        .get(&DataKey::AggregationTipperTips(batch_id, tipper.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn track_tipper_tip(env: &Env, batch_id: u64, tipper: &Address, tip_index: u32) {
    let mut indices: Vec<u32> = env
        .storage()
        .persistent()
        .get(&DataKey::AggregationTipperTips(batch_id, tipper.clone()))
        .unwrap_or_else(|| Vec::new(env));
    indices.push_back(tip_index);
    env.storage().persistent().set(
        &DataKey::AggregationTipperTips(batch_id, tipper.clone()),
        &indices,
    );
}
