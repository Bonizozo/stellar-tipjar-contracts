//! Tip Volatility Index (TVI)
//!
//! Tracks tip-amount fluctuations over time and computes a rolling volatility
//! metric for each creator / token pair.
//!
//! # Design
//!
//! Each index maintains a **ring buffer** of tip-amount observations (identical
//! in structure to the TWAP oracle).  On every new observation the module:
//!
//! 1. Appends the raw tip amount to the ring buffer.
//! 2. Recomputes the **rolling mean** and **rolling variance** over the
//!    configured window using Welford's online algorithm (integer approximation).
//! 3. Derives the **volatility** as the square root of the variance, expressed
//!    in basis points relative to the mean (annualised-style index value).
//! 4. Stores a `VolatilitySnapshot` in persistent history for audit / charting.
//! 5. Emits a `("tvi_upd",)` event so off-chain indexers can react.
//!
//! # Precision
//!
//! All intermediate values are scaled by `PRECISION` (1 000 000 = 1.0) to
//! avoid integer truncation.  The final volatility index is expressed in
//! basis points (10 000 = 100 %).

pub mod calculator;
pub mod history;

use soroban_sdk::{contracttype, Address, Env, Vec};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Fixed-point precision multiplier (1 000 000 = 1.0).
pub const PRECISION: i128 = 1_000_000;

/// Basis-point denominator (10 000 = 100 %).
pub const BPS_DENOM: i128 = 10_000;

/// Default rolling window: 24 observations.
pub const DEFAULT_WINDOW_SIZE: u32 = 24;

/// Minimum window size.
pub const MIN_WINDOW_SIZE: u32 = 2;

/// Maximum ring-buffer / window size.
pub const MAX_WINDOW_SIZE: u32 = 256;

/// Maximum number of snapshots kept in history per index.
pub const MAX_HISTORY: u32 = 512;

/// Minimum time between observations (anti-spam), in seconds.
pub const MIN_OBSERVATION_INTERVAL: u64 = 1;

// ── Data types ───────────────────────────────────────────────────────────────

/// A single tip-amount observation stored in the ring buffer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolObservation {
    /// Ledger timestamp of this observation.
    pub timestamp: u64,
    /// Tip amount recorded (token base units).
    pub amount: i128,
}

/// Live state of a volatility index.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolatilityIndex {
    /// Unique index ID.
    pub index_id: u64,
    /// Creator address this index tracks.
    pub creator: Address,
    /// Token this index tracks.
    pub token: Address,
    /// Rolling window size (number of observations).
    pub window_size: u32,
    /// Ring-buffer write pointer (index of most-recently written slot).
    pub write_index: u32,
    /// Total observations ever recorded (used to detect under-full buffer).
    pub observation_count: u64,
    /// Current rolling mean × PRECISION.
    pub mean: i128,
    /// Current rolling variance × PRECISION (population variance).
    pub variance: i128,
    /// Current volatility index in basis points.
    pub volatility_bps: i128,
    /// Timestamp of the last observation.
    pub last_update: u64,
    /// Whether the index is active.
    pub active: bool,
}

/// A point-in-time snapshot of the volatility index, stored for history.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolatilitySnapshot {
    /// Index this snapshot belongs to.
    pub index_id: u64,
    /// Sequential snapshot number (0-based).
    pub snapshot_seq: u64,
    /// Timestamp of the snapshot.
    pub timestamp: u64,
    /// Mean tip amount × PRECISION at snapshot time.
    pub mean: i128,
    /// Variance × PRECISION at snapshot time.
    pub variance: i128,
    /// Volatility in basis points at snapshot time.
    pub volatility_bps: i128,
    /// Number of observations in the window at snapshot time.
    pub window_observations: u32,
}

/// Global configuration for the volatility module.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolatilityConfig {
    pub default_window_size: u32,
    pub min_observation_interval: u64,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

pub fn get_index(env: &Env, index_id: u64) -> Option<VolatilityIndex> {
    env.storage().persistent().get(&DataKey::VolIndex(index_id))
}

pub fn save_index(env: &Env, idx: &VolatilityIndex) {
    env.storage()
        .persistent()
        .set(&DataKey::VolIndex(idx.index_id), idx);
}

pub fn get_counter(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::VolCounter)
        .unwrap_or(0u64)
}

pub fn next_id(env: &Env) -> u64 {
    let id = get_counter(env) + 1;
    env.storage().persistent().set(&DataKey::VolCounter, &id);
    id
}

pub fn get_observation(env: &Env, index_id: u64, slot: u32) -> Option<VolObservation> {
    env.storage()
        .persistent()
        .get(&DataKey::VolObservation(index_id, slot))
}

pub fn set_observation(env: &Env, index_id: u64, slot: u32, obs: &VolObservation) {
    env.storage()
        .persistent()
        .set(&DataKey::VolObservation(index_id, slot), obs);
}

pub fn get_snapshot(env: &Env, index_id: u64, seq: u64) -> Option<VolatilitySnapshot> {
    env.storage()
        .persistent()
        .get(&DataKey::VolSnapshot(index_id, seq))
}

pub fn save_snapshot(env: &Env, snap: &VolatilitySnapshot) {
    env.storage().persistent().set(
        &DataKey::VolSnapshot(snap.index_id, snap.snapshot_seq),
        snap,
    );
}

pub fn get_snapshot_count(env: &Env, index_id: u64) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::VolSnapshotCount(index_id))
        .unwrap_or(0u64)
}

pub fn increment_snapshot_count(env: &Env, index_id: u64) -> u64 {
    let seq = get_snapshot_count(env, index_id);
    env.storage()
        .persistent()
        .set(&DataKey::VolSnapshotCount(index_id), &(seq + 1));
    seq
}

pub fn get_creator_indices(env: &Env, creator: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::VolCreatorIndices(creator.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn add_creator_index(env: &Env, creator: &Address, index_id: u64) {
    let mut list = get_creator_indices(env, creator);
    if !list.contains(index_id) {
        list.push_back(index_id);
        env.storage()
            .persistent()
            .set(&DataKey::VolCreatorIndices(creator.clone()), &list);
    }
}

pub fn get_config(env: &Env) -> VolatilityConfig {
    env.storage()
        .persistent()
        .get(&DataKey::VolConfig)
        .unwrap_or(VolatilityConfig {
            default_window_size: DEFAULT_WINDOW_SIZE,
            min_observation_interval: MIN_OBSERVATION_INTERVAL,
        })
}

pub fn save_config(env: &Env, cfg: &VolatilityConfig) {
    env.storage().persistent().set(&DataKey::VolConfig, cfg);
}

// ── Ring-buffer helper ───────────────────────────────────────────────────────

/// Returns the slot index that is `offset` steps before `write_index`.
fn ring_back(write_index: u32, offset: u32, capacity: u32) -> u32 {
    (write_index + capacity - (offset % capacity)) % capacity
}

/// Collect up to `window_size` most-recent observations in chronological order.
pub fn collect_window(env: &Env, idx: &VolatilityIndex) -> Vec<VolObservation> {
    let capacity = idx.window_size;
    let available = (idx.observation_count as u32).min(capacity);

    let mut tmp: Vec<VolObservation> = Vec::new(env);
    for offset in 0..available {
        let slot = ring_back(idx.write_index, offset, capacity);
        if let Some(obs) = get_observation(env, idx.index_id, slot) {
            tmp.push_back(obs);
        }
    }

    // Reverse to chronological order
    let len = tmp.len();
    let mut result: Vec<VolObservation> = Vec::new(env);
    for i in 0..len {
        result.push_back(tmp.get(len - 1 - i).unwrap());
    }
    result
}

// ── Core logic ───────────────────────────────────────────────────────────────

/// Create a new volatility index for a (creator, token) pair.
/// Returns the new index ID.
pub fn create_index(env: &Env, creator: &Address, token: &Address, window_size: u32) -> u64 {
    let index_id = next_id(env);
    let now = env.ledger().timestamp();

    let idx = VolatilityIndex {
        index_id,
        creator: creator.clone(),
        token: token.clone(),
        window_size,
        write_index: 0,
        observation_count: 0,
        mean: 0,
        variance: 0,
        volatility_bps: 0,
        last_update: now,
        active: true,
    };

    save_index(env, &idx);
    add_creator_index(env, creator, index_id);

    index_id
}

/// Record a new tip-amount observation and recompute volatility metrics.
/// Returns the updated `VolatilityIndex`.
pub fn record_observation(env: &Env, index_id: u64, amount: i128) -> VolatilityIndex {
    let mut idx = get_index(env, index_id).expect("index not found");
    assert!(idx.active, "index not active");

    let now = env.ledger().timestamp();
    let cfg = get_config(env);
    assert!(
        now >= idx.last_update + cfg.min_observation_interval,
        "observation too frequent"
    );

    // Write new observation into ring buffer
    let next_slot = (idx.write_index + 1) % idx.window_size;
    let write_slot = if idx.observation_count == 0 {
        0
    } else {
        next_slot
    };

    set_observation(
        env,
        index_id,
        write_slot,
        &VolObservation {
            timestamp: now,
            amount,
        },
    );

    idx.write_index = write_slot;
    idx.observation_count += 1;
    idx.last_update = now;

    // Collect the current window and recompute stats
    let window = collect_window(env, &idx);
    let (mean, variance) = calculator::compute_mean_variance(env, &window);
    let vol_bps = calculator::variance_to_volatility_bps(mean, variance);

    idx.mean = mean;
    idx.variance = variance;
    idx.volatility_bps = vol_bps;

    save_index(env, &idx);

    // Persist a snapshot for history
    let seq = increment_snapshot_count(env, index_id);
    let snap = VolatilitySnapshot {
        index_id,
        snapshot_seq: seq,
        timestamp: now,
        mean,
        variance,
        volatility_bps: vol_bps,
        window_observations: window.len(),
    };
    save_snapshot(env, &snap);

    idx
}

/// Deactivate a volatility index. Only the creator may do this.
pub fn deactivate_index(env: &Env, caller: &Address, index_id: u64) {
    let mut idx = get_index(env, index_id).expect("index not found");
    assert!(idx.creator == *caller, "not the index creator");
    idx.active = false;
    save_index(env, &idx);
}
