//! Aggregation batch settlement.
//!
//! Settlement collects all non-refunded queued tips in a batch, deducts the
//! aggregation fee, and credits the creator's balance in a single write.
//! The aggregation fee is added to the platform fee balance for the token.

use soroban_sdk::{panic_with_error, Address, Env};

use crate::{DataKey, TipJarError};

use super::{
    batch::{self, clear_open_batch, is_ready_to_settle, save_batch},
    queue::get_queued_tip,
    BatchStatus, SettlementResult,
};

// ── Public API ───────────────────────────────────────────────────────────────

/// Settles an open aggregation batch.
///
/// Can be called by anyone (creator, tipper, or keeper) once the batch is
/// ready (optimal size reached or time window expired).
///
/// Steps:
/// 1. Validate batch is open and ready.
/// 2. Sum all non-refunded tip amounts.
/// 3. Deduct aggregation fee.
/// 4. Credit creator balance in one write.
/// 5. Accumulate fee into platform fee balance.
/// 6. Mark batch as Settled.
///
/// Returns a [`SettlementResult`] summary.
///
/// Panics if:
/// - Batch not found.
/// - Batch is not Open.
/// - Batch is not yet ready (too small and window not expired).
/// - No non-refunded tips remain.
pub fn settle_batch(env: &Env, caller: &Address, batch_id: u64) -> SettlementResult {
    caller.require_auth();

    let mut agg_batch: super::AggregationBatch = env
        .storage()
        .persistent()
        .get(&DataKey::AggregationBatch(batch_id))
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AggregationBatchNotFound));

    if agg_batch.status != BatchStatus::Open {
        panic_with_error!(env, TipJarError::AggregationBatchAlreadySettled);
    }

    if !is_ready_to_settle(env, &agg_batch) {
        panic_with_error!(env, TipJarError::AggregationBatchNotReady);
    }

    // Sum all non-refunded tips
    let mut gross_total: i128 = 0;
    let mut included_count: u32 = 0;

    for i in 0..agg_batch.tip_count {
        if let Some(tip) = get_queued_tip(env, batch_id, i) {
            if !tip.refunded {
                gross_total = gross_total
                    .checked_add(tip.amount)
                    .expect("settlement overflow");
                included_count += 1;
            }
        }
    }

    if included_count == 0 || gross_total == 0 {
        // All tips were cancelled — mark as cancelled
        agg_batch.status = BatchStatus::Cancelled;
        agg_batch.closed_at = Some(env.ledger().timestamp());
        save_batch(env, &agg_batch);
        clear_open_batch(env, &agg_batch.token, &agg_batch.creator);

        env.events().publish(
            (soroban_sdk::symbol_short!("agg_cncl"),),
            (batch_id, soroban_sdk::symbol_short!("empty")),
        );

        return SettlementResult {
            batch_id,
            tip_count: 0,
            creator_amount: 0,
            fee_amount: 0,
            settled_at: env.ledger().timestamp(),
        };
    }

    let config = batch::get_config(env);

    // Calculate aggregation fee
    let fee_amount = gross_total * config.fee_bps as i128 / 10_000;
    let creator_amount = gross_total - fee_amount;

    // Credit creator balance (single write)
    let bal_key = DataKey::CreatorBalance(agg_batch.creator.clone(), agg_batch.token.clone());
    let existing_bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
    env.storage().persistent().set(
        &bal_key,
        &existing_bal
            .checked_add(creator_amount)
            .expect("balance overflow"),
    );

    // Update creator total received
    let tot_key = DataKey::CreatorTotal(agg_batch.creator.clone(), agg_batch.token.clone());
    let existing_tot: i128 = env.storage().persistent().get(&tot_key).unwrap_or(0);
    env.storage().persistent().set(
        &tot_key,
        &existing_tot
            .checked_add(gross_total)
            .expect("total overflow"),
    );

    // Accumulate aggregation fee into platform fee balance
    if fee_amount > 0 {
        let fee_key = DataKey::PlatformFeeBalance(agg_batch.token.clone());
        let existing_fee: i128 = env
            .storage()
            .instance()
            .get(&fee_key)
            .unwrap_or(0i128);
        env.storage().instance().set(
            &fee_key,
            &existing_fee
                .checked_add(fee_amount)
                .expect("fee overflow"),
        );
    }

    let now = env.ledger().timestamp();

    // Mark batch as Settled
    agg_batch.status = BatchStatus::Settled;
    agg_batch.closed_at = Some(now);
    agg_batch.fee_collected = fee_amount;
    save_batch(env, &agg_batch);

    // Clear the open-batch pointer so a new batch can be opened
    clear_open_batch(env, &agg_batch.token, &agg_batch.creator);

    let result = SettlementResult {
        batch_id,
        tip_count: included_count,
        creator_amount,
        fee_amount,
        settled_at: now,
    };

    env.events().publish(
        (soroban_sdk::symbol_short!("agg_setl"),),
        (
            agg_batch.creator.clone(),
            agg_batch.token.clone(),
            batch_id,
            included_count,
            creator_amount,
            fee_amount,
        ),
    );

    result
}

/// Force-cancels an open batch and refunds all non-refunded tips.
///
/// Admin only. Used for emergency cleanup.
///
/// Returns the number of tips refunded.
pub fn cancel_batch(env: &Env, admin: &Address, batch_id: u64) -> u32 {
    admin.require_auth();
    require_admin(env, admin);

    let mut agg_batch: super::AggregationBatch = env
        .storage()
        .persistent()
        .get(&DataKey::AggregationBatch(batch_id))
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AggregationBatchNotFound));

    if agg_batch.status != BatchStatus::Open {
        panic_with_error!(env, TipJarError::AggregationBatchAlreadySettled);
    }

    let token_client = soroban_sdk::token::Client::new(env, &agg_batch.token);
    let contract = env.current_contract_address();
    let mut refunded_count: u32 = 0;

    for i in 0..agg_batch.tip_count {
        if let Some(mut tip) = get_queued_tip(env, batch_id, i) {
            if !tip.refunded {
                tip.refunded = true;
                env.storage()
                    .persistent()
                    .set(&DataKey::AggregationQueuedTip(batch_id, i), &tip);
                token_client.transfer(&contract, &tip.tipper, &tip.amount);
                refunded_count += 1;
            }
        }
    }

    agg_batch.status = BatchStatus::Cancelled;
    agg_batch.closed_at = Some(env.ledger().timestamp());
    save_batch(env, &agg_batch);
    clear_open_batch(env, &agg_batch.token, &agg_batch.creator);

    env.events().publish(
        (soroban_sdk::symbol_short!("agg_adm"),),
        (admin.clone(), batch_id, refunded_count),
    );

    refunded_count
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn require_admin(env: &Env, admin: &Address) {
    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::Unauthorized));
    if *admin != stored_admin {
        panic_with_error!(env, TipJarError::Unauthorized);
    }
}
