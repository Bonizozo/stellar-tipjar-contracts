//! TWAP Oracle — Manipulation-Resistant Price Feeds
//!
//! Implements a Time-Weighted Average Price (TWAP) oracle for tip tokens.
//!
//! ## Design
//!
//! Each oracle tracks a **ring buffer** of price observations.  Every call to
//! `record_price` appends a new `Observation` (timestamp + cumulative price
//! accumulator).  The TWAP over any window `[t0, t1]` is then:
//!
//! ```text
//! TWAP = (accumulator[t1] - accumulator[t0]) / (t1 - t0)
//! ```
//!
//! This is the same approach used by Uniswap v2/v3 and is resistant to
//! single-block price manipulation because an attacker would need to sustain
//! a manipulated price for the entire observation window.
//!
//! ## Storage layout
//!
//! - `TwapOracle(oracle_id)`          — oracle config + live state
//! - `TwapObservation(oracle_id, idx)` — individual ring-buffer slot
//! - `TwapOracleCounter`              — global ID counter
//!
//! ## Precision
//!
//! Prices are stored × `PRICE_PRECISION` (1e7).  Accumulators are
//! `price × PRICE_PRECISION × elapsed_seconds`, so they can grow large for
//! long-lived oracles; `i128` gives ~170 bits of headroom which is sufficient
//! for decades of operation at reasonable prices.

use soroban_sdk::{contracttype, panic_with_error, symbol_short, Address, Env, Vec};

use crate::{DataKey, TipJarError};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Fixed-point precision for prices: 1e7.
pub const PRICE_PRECISION: i128 = 10_000_000;

/// Default observation window: 30 minutes.
pub const DEFAULT_WINDOW_SECONDS: u64 = 1_800;

/// Minimum observation window: 1 minute.
pub const MIN_WINDOW_SECONDS: u64 = 60;

/// Maximum observation window: 7 days.
pub const MAX_WINDOW_SECONDS: u64 = 7 * 24 * 3600;

/// Maximum ring-buffer capacity per oracle.
pub const MAX_OBSERVATIONS: u32 = 256;

/// Minimum ring-buffer capacity.
pub const MIN_OBSERVATIONS: u32 = 2;

/// Minimum time between price updates (anti-spam): 1 second.
pub const MIN_UPDATE_INTERVAL: u64 = 1;

// ── Data types ────────────────────────────────────────────────────────────────

/// A single price observation stored in the ring buffer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    /// Ledger timestamp when this observation was recorded.
    pub timestamp: u64,
    /// Cumulative price accumulator at this timestamp.
    /// `accumulator[n] = accumulator[n-1] + price[n-1] × (timestamp[n] - timestamp[n-1])`
    pub price_cumulative: i128,
    /// Spot price recorded at this timestamp × PRICE_PRECISION.
    pub price: i128,
}

/// Oracle configuration and live state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TwapOracle {
    /// Unique oracle identifier.
    pub id: u64,
    /// Address authorised to push price updates.
    pub updater: Address,
    /// Base token of the price pair (e.g. TIP token).
    pub base_token: Address,
    /// Quote token of the price pair (e.g. USDC).
    pub quote_token: Address,
    /// TWAP observation window in seconds.
    pub window_seconds: u64,
    /// Ring-buffer capacity (number of observations stored).
    pub max_observations: u32,
    /// Index of the most-recently written slot (wraps around).
    pub write_index: u32,
    /// Total number of observations ever written (used to detect under-full buffer).
    pub observation_count: u64,
    /// Latest spot price × PRICE_PRECISION.
    pub last_price: i128,
    /// Timestamp of the last price update.
    pub last_update: u64,
    /// Whether the oracle is active.
    pub active: bool,
}

/// Snapshot returned by `get_twap`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TwapResult {
    /// TWAP over the requested window × PRICE_PRECISION.
    pub twap: i128,
    /// Actual window used in seconds (may be shorter if not enough history).
    pub window_used: u64,
    /// Oldest observation timestamp included in the calculation.
    pub oldest_timestamp: u64,
    /// Newest observation timestamp included in the calculation.
    pub newest_timestamp: u64,
    /// Number of observations used.
    pub observations_used: u32,
}

// ── Storage helpers ───────────────────────────────────────────────────────────

fn get_oracle(env: &Env, oracle_id: u64) -> Option<TwapOracle> {
    env.storage()
        .persistent()
        .get(&DataKey::TwapOracle(oracle_id))
}

fn get_oracle_or_panic(env: &Env, oracle_id: u64) -> TwapOracle {
    get_oracle(env, oracle_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::TwapOracleNotFound))
}

fn set_oracle(env: &Env, oracle: &TwapOracle) {
    env.storage()
        .persistent()
        .set(&DataKey::TwapOracle(oracle.id), oracle);
}

fn get_observation(env: &Env, oracle_id: u64, idx: u32) -> Option<Observation> {
    env.storage()
        .persistent()
        .get(&DataKey::TwapObservation(oracle_id, idx))
}

fn set_observation(env: &Env, oracle_id: u64, idx: u32, obs: &Observation) {
    env.storage()
        .persistent()
        .set(&DataKey::TwapObservation(oracle_id, idx), obs);
}

fn next_oracle_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::TwapOracleCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .persistent()
        .set(&DataKey::TwapOracleCounter, &next);
    next
}

// ── Ring-buffer helpers ───────────────────────────────────────────────────────

/// Returns the slot index that is `offset` steps before `write_index`.
fn ring_index_back(write_index: u32, offset: u32, capacity: u32) -> u32 {
    (write_index + capacity - (offset % capacity)) % capacity
}

/// Collects up to `max_observations` observations going backwards from the
/// current write position, stopping when we exceed `window_seconds`.
/// Returns observations in chronological order (oldest first).
fn collect_window(env: &Env, oracle: &TwapOracle, window_seconds: u64) -> Vec<Observation> {
    let mut result: Vec<Observation> = Vec::new(env);

    let total_written = oracle.observation_count;
    if total_written == 0 {
        return result;
    }

    let capacity = oracle.max_observations;
    // How many slots are actually populated?
    let available = (total_written as u32).min(capacity);

    // Walk backwards from the most-recent slot
    let newest_obs = match get_observation(env, oracle.id, oracle.write_index) {
        Some(o) => o,
        None => return result,
    };
    let cutoff = newest_obs.timestamp.saturating_sub(window_seconds);

    // Collect into a temporary buffer (newest → oldest), then reverse
    let mut tmp: Vec<Observation> = Vec::new(env);
    for offset in 0..available {
        let idx = ring_index_back(oracle.write_index, offset, capacity);
        if let Some(obs) = get_observation(env, oracle.id, idx) {
            if obs.timestamp < cutoff {
                break;
            }
            tmp.push_back(obs);
        }
    }

    // Reverse to get chronological order
    let len = tmp.len();
    for i in 0..len {
        result.push_back(tmp.get(len - 1 - i).unwrap());
    }

    result
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Creates a new TWAP oracle. Returns the oracle ID.
///
/// * `updater`          — address authorised to push price updates.
/// * `window_seconds`   — default TWAP window (60 – 604 800 s).
/// * `max_observations` — ring-buffer size (2 – 256).
/// * `initial_price`    — seed price × PRICE_PRECISION (must be > 0).
pub fn create_oracle(
    env: &Env,
    creator: &Address,
    updater: &Address,
    base_token: &Address,
    quote_token: &Address,
    window_seconds: u64,
    max_observations: u32,
    initial_price: i128,
) -> u64 {
    creator.require_auth();

    if !(MIN_WINDOW_SECONDS..=MAX_WINDOW_SECONDS).contains(&window_seconds) {
        panic_with_error!(env, TipJarError::TwapInvalidWindow);
    }
    if !(MIN_OBSERVATIONS..=MAX_OBSERVATIONS).contains(&max_observations) {
        panic_with_error!(env, TipJarError::TwapInvalidParams);
    }
    if initial_price <= 0 {
        panic_with_error!(env, TipJarError::TwapInvalidPrice);
    }

    let oracle_id = next_oracle_id(env);
    let now = env.ledger().timestamp();

    // Seed the ring buffer with the initial observation
    let seed = Observation {
        timestamp: now,
        price_cumulative: 0,
        price: initial_price,
    };
    set_observation(env, oracle_id, 0, &seed);

    let oracle = TwapOracle {
        id: oracle_id,
        updater: updater.clone(),
        base_token: base_token.clone(),
        quote_token: quote_token.clone(),
        window_seconds,
        max_observations,
        write_index: 0,
        observation_count: 1,
        last_price: initial_price,
        last_update: now,
        active: true,
    };

    set_oracle(env, &oracle);

    env.events().publish(
        (symbol_short!("twap_new"),),
        (
            oracle_id,
            base_token.clone(),
            quote_token.clone(),
            initial_price,
        ),
    );

    oracle_id
}

/// Records a new price observation.
///
/// Only the oracle's `updater` address may call this.
/// Advances the ring-buffer write pointer and updates the cumulative accumulator.
pub fn record_price(env: &Env, updater: &Address, oracle_id: u64, price: i128) {
    updater.require_auth();

    if price <= 0 {
        panic_with_error!(env, TipJarError::TwapInvalidPrice);
    }

    let mut oracle = get_oracle_or_panic(env, oracle_id);

    if !oracle.active {
        panic_with_error!(env, TipJarError::TwapOracleInactive);
    }
    if oracle.updater != *updater {
        panic_with_error!(env, TipJarError::Unauthorized);
    }

    let now = env.ledger().timestamp();
    if now < oracle.last_update + MIN_UPDATE_INTERVAL {
        panic_with_error!(env, TipJarError::TwapUpdateTooFrequent);
    }

    // Fetch the previous observation to compute the new accumulator
    let prev = get_observation(env, oracle_id, oracle.write_index).unwrap_or(Observation {
        timestamp: now,
        price_cumulative: 0,
        price,
    });

    let elapsed = now.saturating_sub(prev.timestamp);
    let new_accumulator = prev
        .price_cumulative
        .saturating_add(prev.price.saturating_mul(elapsed as i128));

    // Advance write pointer (ring buffer)
    let next_idx = (oracle.write_index + 1) % oracle.max_observations;

    let new_obs = Observation {
        timestamp: now,
        price_cumulative: new_accumulator,
        price,
    };

    set_observation(env, oracle_id, next_idx, &new_obs);

    oracle.write_index = next_idx;
    oracle.observation_count += 1;
    oracle.last_price = price;
    oracle.last_update = now;

    set_oracle(env, &oracle);

    env.events().publish(
        (symbol_short!("twap_upd"),),
        (oracle_id, price, now, new_accumulator),
    );
}

/// Calculates the TWAP over the oracle's configured window.
///
/// Uses the cumulative price accumulators:
/// `TWAP = Δaccumulator / Δtime`
///
/// If fewer than 2 observations exist within the window, returns the last
/// known spot price as a best-effort value.
pub fn get_twap(env: &Env, oracle_id: u64) -> TwapResult {
    get_twap_with_window(env, oracle_id, 0) // 0 = use oracle's default window
}

/// Calculates the TWAP over a custom `window_seconds`.
/// Pass `window_seconds = 0` to use the oracle's configured default.
pub fn get_twap_with_window(env: &Env, oracle_id: u64, window_seconds: u64) -> TwapResult {
    let oracle = get_oracle_or_panic(env, oracle_id);

    let window = if window_seconds == 0 {
        oracle.window_seconds
    } else {
        window_seconds
            .max(MIN_WINDOW_SECONDS)
            .min(MAX_WINDOW_SECONDS)
    };

    let observations = collect_window(env, &oracle, window);
    let n = observations.len();

    // Need at least 2 observations to compute a meaningful TWAP
    if n < 2 {
        return TwapResult {
            twap: oracle.last_price,
            window_used: 0,
            oldest_timestamp: oracle.last_update,
            newest_timestamp: oracle.last_update,
            observations_used: n,
        };
    }

    let oldest = observations.get(0).unwrap();
    let newest = observations.get(n - 1).unwrap();

    let time_delta = newest.timestamp.saturating_sub(oldest.timestamp);
    if time_delta == 0 {
        return TwapResult {
            twap: oracle.last_price,
            window_used: 0,
            oldest_timestamp: oldest.timestamp,
            newest_timestamp: newest.timestamp,
            observations_used: n,
        };
    }

    let acc_delta = newest
        .price_cumulative
        .saturating_sub(oldest.price_cumulative);

    // TWAP = Δaccumulator / Δtime
    // accumulator is already price × elapsed, so dividing by elapsed gives price
    let twap = acc_delta / time_delta as i128;

    TwapResult {
        twap,
        window_used: time_delta,
        oldest_timestamp: oldest.timestamp,
        newest_timestamp: newest.timestamp,
        observations_used: n,
    }
}

/// Returns the latest spot price for an oracle (not time-weighted).
pub fn get_latest_price(env: &Env, oracle_id: u64) -> i128 {
    let oracle = get_oracle_or_panic(env, oracle_id);
    oracle.last_price
}

/// Returns up to `limit` most-recent observations for an oracle.
pub fn get_observations(env: &Env, oracle_id: u64, limit: u32) -> Vec<Observation> {
    let oracle = get_oracle_or_panic(env, oracle_id);

    let capacity = oracle.max_observations;
    let available = (oracle.observation_count as u32).min(capacity);
    let count = limit.min(available);

    let mut result: Vec<Observation> = Vec::new(env);
    // Collect newest → oldest, then reverse
    let mut tmp: Vec<Observation> = Vec::new(env);
    for offset in 0..count {
        let idx = ring_index_back(oracle.write_index, offset, capacity);
        if let Some(obs) = get_observation(env, oracle_id, idx) {
            tmp.push_back(obs);
        }
    }
    let len = tmp.len();
    for i in 0..len {
        result.push_back(tmp.get(len - 1 - i).unwrap());
    }
    result
}

/// Returns the oracle configuration and state.
pub fn get_oracle_info(env: &Env, oracle_id: u64) -> TwapOracle {
    get_oracle_or_panic(env, oracle_id)
}

/// Updates the oracle's TWAP window and/or updater address.
/// Only the current updater may call this.
pub fn update_config(
    env: &Env,
    updater: &Address,
    oracle_id: u64,
    new_window_seconds: u64,
    new_updater: &Address,
) {
    updater.require_auth();

    let mut oracle = get_oracle_or_panic(env, oracle_id);

    if oracle.updater != *updater {
        panic_with_error!(env, TipJarError::Unauthorized);
    }
    if !(MIN_WINDOW_SECONDS..=MAX_WINDOW_SECONDS).contains(&new_window_seconds) {
        panic_with_error!(env, TipJarError::TwapInvalidWindow);
    }

    oracle.window_seconds = new_window_seconds;
    oracle.updater = new_updater.clone();
    set_oracle(env, &oracle);

    env.events().publish(
        (symbol_short!("twap_cfg"),),
        (oracle_id, new_window_seconds, new_updater.clone()),
    );
}

/// Deactivates an oracle. Only the updater may call this.
pub fn deactivate_oracle(env: &Env, updater: &Address, oracle_id: u64) {
    updater.require_auth();

    let mut oracle = get_oracle_or_panic(env, oracle_id);

    if oracle.updater != *updater {
        panic_with_error!(env, TipJarError::Unauthorized);
    }
    if !oracle.active {
        panic_with_error!(env, TipJarError::TwapOracleInactive);
    }

    oracle.active = false;
    set_oracle(env, &oracle);

    env.events()
        .publish((symbol_short!("twap_off"),), (oracle_id,));
}
