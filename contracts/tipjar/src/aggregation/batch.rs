//! Batch lifecycle management.
//!
//! Handles creating, retrieving, and tracking aggregation batches.
//! Each `(token, creator)` pair has at most one open batch at a time.

use soroban_sdk::{Address, Env};

use crate::DataKey;

use super::{AggregationBatch, AggregationConfig, BatchStatus};

// ── Public API ───────────────────────────────────────────────────────────────

/// Returns the aggregation configuration, falling back to defaults.
pub fn get_config(env: &Env) -> AggregationConfig {
    env.storage()
        .instance()
        .get(&DataKey::AggregationConfig)
        .unwrap_or_default()
}

/// Persists the aggregation configuration. Admin only (caller must verify).
pub fn set_config(env: &Env, config: &AggregationConfig) {
    env.storage()
        .instance()
        .set(&DataKey::AggregationConfig, config);
}

/// Returns the open batch for `(token, creator)`, or `None`.
pub fn get_open_batch(env: &Env, token: &Address, creator: &Address) -> Option<AggregationBatch> {
    let batch_id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::AggregationOpenBatch(token.clone(), creator.clone()))?;
    env.storage()
        .persistent()
        .get(&DataKey::AggregationBatch(batch_id))
}

/// Returns a batch by ID, or `None`.
pub fn get_batch(env: &Env, batch_id: u64) -> Option<AggregationBatch> {
    env.storage()
        .persistent()
        .get(&DataKey::AggregationBatch(batch_id))
}

/// Saves a batch record.
pub fn save_batch(env: &Env, batch: &AggregationBatch) {
    env.storage()
        .persistent()
        .set(&DataKey::AggregationBatch(batch.id), batch);
}

/// Opens a new batch for `(token, creator)` and returns it.
///
/// Increments the global batch counter and registers the batch as the open
/// batch for this pair.
pub fn open_batch(env: &Env, token: &Address, creator: &Address) -> AggregationBatch {
    let batch_id = next_batch_id(env);
    let batch = AggregationBatch {
        id: batch_id,
        token: token.clone(),
        creator: creator.clone(),
        total_queued: 0,
        tip_count: 0,
        status: BatchStatus::Open,
        opened_at: env.ledger().timestamp(),
        closed_at: None,
        fee_collected: 0,
    };
    save_batch(env, &batch);
    env.storage().persistent().set(
        &DataKey::AggregationOpenBatch(token.clone(), creator.clone()),
        &batch_id,
    );
    batch
}

/// Closes the open-batch pointer for `(token, creator)`.
pub fn clear_open_batch(env: &Env, token: &Address, creator: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::AggregationOpenBatch(token.clone(), creator.clone()));
}

/// Returns the total number of batches ever created.
pub fn batch_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::AggregationBatchCounter)
        .unwrap_or(0)
}

/// Computes the optimal batch size for the current configuration.
///
/// Returns `config.optimal_batch_size`. This function is the extension point
/// for more sophisticated heuristics (e.g. dynamic sizing based on gas price
/// oracle data).
pub fn optimal_batch_size(env: &Env) -> u32 {
    get_config(env).optimal_batch_size
}

/// Returns `true` if `batch` is ready for settlement:
/// - tip count has reached the optimal size, OR
/// - the batch window has expired.
pub fn is_ready_to_settle(env: &Env, batch: &AggregationBatch) -> bool {
    let config = get_config(env);
    let now = env.ledger().timestamp();
    batch.tip_count >= config.optimal_batch_size
        || now >= batch.opened_at + config.batch_window_seconds
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn next_batch_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::AggregationBatchCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .persistent()
        .set(&DataKey::AggregationBatchCounter, &next);
    next
}
