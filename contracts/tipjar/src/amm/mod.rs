//! Automated Market Maker for Tip Tokens
//!
//! This module provides token swap and liquidity pool functionality.

pub mod pool;
pub mod swap;
pub mod pricing;

use soroban_sdk::{contracttype, Address, Env};

/// Liquidity pool structure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidityPool {
    pub token_a: Address,
    pub token_b: Address,
    pub reserve_a: i128,
    pub reserve_b: i128,
    pub total_shares: i128,
    pub fee_bps: u32, // Fee in basis points (e.g., 30 = 0.3%)
}

/// Liquidity provider share
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidityShare {
    pub provider: Address,
    pub shares: i128,
    pub pool_id: u64,
}

/// Swap result
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapResult {
    pub amount_out: i128,
    pub fee_amount: i128,
    pub new_reserve_a: i128,
    pub new_reserve_b: i128,
}

/// Pool creation parameters
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolParams {
    pub token_a: Address,
    pub token_b: Address,
    pub initial_liquidity_a: i128,
    pub initial_liquidity_b: i128,
    pub fee_bps: u32,
}

/// Default fee: 0.3% (30 basis points)
pub const DEFAULT_FEE_BPS: u32 = 30;

/// Storage keys for AMM
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Pool(u64),
    PoolCounter,
    PoolByTokens(Address, Address),
    LiquidityShare(u64, Address),
    UserShares(u64),
}

/// Create a new liquidity pool
pub fn create_pool(
    env: &Env,
    token_a: &Address,
    token_b: &Address,
    fee_bps: Option<u32>,
) -> u64 {
    // Check if pool already exists
    let pool_id = get_pool_id(env, token_a, token_b);
    if pool_id.is_some() {
        panic!("Pool already exists for this token pair");
    }

    let counter_key = DataKey::PoolCounter;
    let next_id: u64 = env.storage().persistent().get(&counter_key).unwrap_or(0);
    let new_id = next_id + 1;

    let pool = LiquidityPool {
        token_a: token_a.clone(),
        token_b: token_b.clone(),
        reserve_a: 0,
        reserve_b: 0,
        total_shares: 0,
        fee_bps: fee_bps.unwrap_or(DEFAULT_FEE_BPS),
    };

    env.storage().persistent().set(&DataKey::Pool(new_id), &pool);
    env.storage().persistent().set(&counter_key, &new_id);

    // Store pool ID for token pair lookup
    let pair_key = DataKey::PoolByTokens(token_a.clone(), token_b.clone());
    env.storage().persistent().set(&pair_key, &new_id);

    // Also store reverse pair
    let reverse_pair_key = DataKey::PoolByTokens(token_b.clone(), token_a.clone());
    env.storage().persistent().set(&reverse_pair_key, &new_id);

    new_id
}

/// Get pool ID for token pair
pub fn get_pool_id(env: &Env, token_a: &Address, token_b: &Address) -> Option<u64> {
    let pair_key = DataKey::PoolByTokens(token_a.clone(), token_b.clone());
    env.storage().persistent().get(&pair_key)
}

/// Get pool by ID
pub fn get_pool(env: &Env, pool_id: u64) -> Option<LiquidityPool> {
    let pool_key = DataKey::Pool(pool_id);
    env.storage().persistent().get(&pool_key)
}

/// Get pool by token pair
pub fn get_pool_by_tokens(env: &Env, token_a: &Address, token_b: &Address) -> Option<LiquidityPool> {
    let pool_id = get_pool_id(env, token_a, token_b)?;
    get_pool(env, pool_id)
}

/// Update pool state
pub fn update_pool(env: &Env, pool_id: u64, pool: &LiquidityPool) {
    let pool_key = DataKey::Pool(pool_id);
    env.storage().persistent().set(&pool_key, pool);
}

/// Get user's liquidity shares
pub fn get_user_shares(env: &Env, pool_id: u64, user: &Address) -> i128 {
    let share_key = DataKey::LiquidityShare(pool_id, user.clone());
    env.storage().persistent().get(&share_key).unwrap_or(0)
}

/// Update user's liquidity shares
pub fn update_user_shares(env: &Env, pool_id: u64, user: &Address, shares: i128) {
    let share_key = DataKey::LiquidityShare(pool_id, user.clone());
    env.storage().persistent().set(&share_key, &shares);
}
