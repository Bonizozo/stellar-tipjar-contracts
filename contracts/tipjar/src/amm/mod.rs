//! Automated Market Maker (AMM) for Tip Token Swaps
//!
//! Implements a constant-product AMM (x·y = k) with:
//! - Permissionless liquidity pool creation per token pair
//! - Proportional LP share minting / burning
//! - Swap with configurable fee and slippage protection
//! - Accumulated fee rewards claimable by LPs
//!
//! # Storage layout (all keys in top-level `DataKey`)
//! - `AmmPool(pool_id)`                  — pool state
//! - `AmmPoolCounter`                    — global pool ID counter
//! - `AmmPoolByTokens(token_a, token_b)` — pool ID lookup by pair
//! - `AmmLpShares(pool_id, provider)`    — LP share balance
//! - `AmmLpRewards(pool_id, provider)`   — unclaimed fee rewards per provider
//! - `AmmPoolFeeAccum(pool_id)`          — cumulative fees per share (× PRECISION)
//! - `AmmProviderDebt(pool_id, provider)`— fee-per-share snapshot at last claim

pub mod pool;
pub mod pricing;
pub mod swap;

use soroban_sdk::{contracttype, Address, Env};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Default swap fee: 0.3 % (30 bps).
pub const DEFAULT_FEE_BPS: u32 = 30;

/// Maximum allowed fee: 10 % (1 000 bps).
pub const MAX_FEE_BPS: u32 = 1_000;

/// Minimum initial liquidity for each token when creating a pool.
pub const MIN_INITIAL_LIQUIDITY: i128 = 1_000;

/// Fixed-point precision for fee accumulator (1 000 000 = 1.0).
pub const PRECISION: i128 = 1_000_000;

// ── Data types ───────────────────────────────────────────────────────────────

/// State of a liquidity pool.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidityPool {
    /// Unique pool ID.
    pub pool_id: u64,
    /// First token in the pair (canonical order: lower address first).
    pub token_a: Address,
    /// Second token in the pair.
    pub token_b: Address,
    /// Reserve of token A held by the pool.
    pub reserve_a: i128,
    /// Reserve of token B held by the pool.
    pub reserve_b: i128,
    /// Total LP shares outstanding.
    pub total_shares: i128,
    /// Swap fee in basis points.
    pub fee_bps: u32,
    /// Cumulative fee-per-share accumulator × PRECISION (token A equivalent).
    pub fee_per_share_accum: i128,
    /// Total fees collected (token A equivalent) since pool creation.
    pub total_fees_collected: i128,
}

/// Result returned from a swap operation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapResult {
    /// Amount of output token received.
    pub amount_out: i128,
    /// Fee charged on the input amount.
    pub fee_amount: i128,
    /// Pool reserve of token A after the swap.
    pub new_reserve_a: i128,
    /// Pool reserve of token B after the swap.
    pub new_reserve_b: i128,
    /// Price impact in basis points.
    pub price_impact_bps: i128,
}

/// Result returned from adding liquidity.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddLiquidityResult {
    /// LP shares minted.
    pub shares_minted: i128,
    /// Actual amount of token A deposited.
    pub amount_a: i128,
    /// Actual amount of token B deposited.
    pub amount_b: i128,
}

/// Result returned from removing liquidity.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoveLiquidityResult {
    /// Amount of token A returned.
    pub amount_a: i128,
    /// Amount of token B returned.
    pub amount_b: i128,
    /// Fee rewards claimed at the same time.
    pub rewards_claimed: i128,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

pub fn get_pool(env: &Env, pool_id: u64) -> Option<LiquidityPool> {
    env.storage().persistent().get(&DataKey::AmmPool(pool_id))
}

pub fn save_pool(env: &Env, pool: &LiquidityPool) {
    env.storage()
        .persistent()
        .set(&DataKey::AmmPool(pool.pool_id), pool);
}

pub fn get_pool_counter(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::AmmPoolCounter)
        .unwrap_or(0u64)
}

pub fn next_pool_id(env: &Env) -> u64 {
    let id = get_pool_counter(env) + 1;
    env.storage()
        .persistent()
        .set(&DataKey::AmmPoolCounter, &id);
    id
}

pub fn get_pool_id_by_tokens(env: &Env, token_a: &Address, token_b: &Address) -> Option<u64> {
    // Try canonical order first, then reverse
    env.storage()
        .persistent()
        .get(&DataKey::AmmPoolByTokens(token_a.clone(), token_b.clone()))
        .or_else(|| {
            env.storage()
                .persistent()
                .get(&DataKey::AmmPoolByTokens(token_b.clone(), token_a.clone()))
        })
}

pub fn register_pool_tokens(env: &Env, pool_id: u64, token_a: &Address, token_b: &Address) {
    env.storage().persistent().set(
        &DataKey::AmmPoolByTokens(token_a.clone(), token_b.clone()),
        &pool_id,
    );
}

pub fn get_lp_shares(env: &Env, pool_id: u64, provider: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::AmmLpShares(pool_id, provider.clone()))
        .unwrap_or(0i128)
}

pub fn set_lp_shares(env: &Env, pool_id: u64, provider: &Address, shares: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::AmmLpShares(pool_id, provider.clone()), &shares);
}

/// Cumulative fee-per-share at the time the provider last claimed.
pub fn get_provider_debt(env: &Env, pool_id: u64, provider: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::AmmProviderDebt(pool_id, provider.clone()))
        .unwrap_or(0i128)
}

pub fn set_provider_debt(env: &Env, pool_id: u64, provider: &Address, debt: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::AmmProviderDebt(pool_id, provider.clone()), &debt);
}

// ── Fee reward helpers ───────────────────────────────────────────────────────

/// Compute pending fee rewards for a provider.
///
/// `pending = shares × (pool.fee_per_share_accum - provider_debt) / PRECISION`
pub fn pending_rewards(env: &Env, pool_id: u64, provider: &Address) -> i128 {
    let pool = match get_pool(env, pool_id) {
        Some(p) => p,
        None => return 0,
    };
    let shares = get_lp_shares(env, pool_id, provider);
    if shares == 0 {
        return 0;
    }
    let debt = get_provider_debt(env, pool_id, provider);
    let delta = pool.fee_per_share_accum - debt;
    if delta <= 0 {
        return 0;
    }
    shares * delta / PRECISION
}

/// Settle pending rewards for a provider and reset their debt snapshot.
/// Returns the amount of rewards settled.
pub fn settle_rewards(env: &Env, pool_id: u64, provider: &Address) -> i128 {
    let pool = match get_pool(env, pool_id) {
        Some(p) => p,
        None => return 0,
    };
    let shares = get_lp_shares(env, pool_id, provider);
    let debt = get_provider_debt(env, pool_id, provider);
    let delta = pool.fee_per_share_accum - debt;
    let reward = if shares > 0 && delta > 0 {
        shares * delta / PRECISION
    } else {
        0
    };
    // Sync debt to current accumulator
    set_provider_debt(env, pool_id, provider, pool.fee_per_share_accum);
    reward
}

/// Accrue a fee amount into the pool's fee-per-share accumulator.
pub fn accrue_fee(_env: &Env, pool: &mut LiquidityPool, fee_amount: i128) {
    if pool.total_shares > 0 && fee_amount > 0 {
        pool.fee_per_share_accum += fee_amount * PRECISION / pool.total_shares;
        pool.total_fees_collected += fee_amount;
    }
}

// ── Integer square root ──────────────────────────────────────────────────────

/// Floor integer square root via Newton's method.
pub fn isqrt(n: i128) -> i128 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}
