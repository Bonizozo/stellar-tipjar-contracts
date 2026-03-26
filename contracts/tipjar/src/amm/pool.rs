//! Liquidity pool operations

use super::{LiquidityPool, DataKey};
use soroban_sdk::{token, Address, Env};

/// Add liquidity to a pool
pub fn add_liquidity(
    env: &Env,
    pool_id: u64,
    provider: &Address,
    amount_a: i128,
    amount_b: i128,
) -> i128 {
    provider.require_auth();

    if amount_a <= 0 || amount_b <= 0 {
        panic!("Amounts must be positive");
    }

    let pool_key = DataKey::Pool(pool_id);
    let mut pool: LiquidityPool = env
        .storage()
        .persistent()
        .get(&pool_key)
        .expect("Pool not found");

    // Calculate shares
    let shares = if pool.total_shares == 0 {
        // Initial liquidity - use geometric mean
        (amount_a * amount_b).sqrt()
    } else {
        // Subsequent liquidity - use minimum ratio to maintain pool balance
        let share_a = amount_a * pool.total_shares / pool.reserve_a;
        let share_b = amount_b * pool.total_shares / pool.reserve_b;
        if share_a < share_b {
            share_a
        } else {
            share_b
        }
    };

    if shares <= 0 {
        panic!("Invalid share amount");
    }

    // Transfer tokens from provider to pool
    let token_client_a = token::Client::new(env, &pool.token_a);
    let token_client_b = token::Client::new(env, &pool.token_b);
    let contract_address = env.current_contract_address();

    token_client_a.transfer(provider, &contract_address, &amount_a);
    token_client_b.transfer(provider, &contract_address, &amount_b);

    // Update pool reserves
    pool.reserve_a += amount_a;
    pool.reserve_b += amount_b;
    pool.total_shares += shares;

    env.storage().persistent().set(&pool_key, &pool);

    // Update user shares
    let share_key = DataKey::LiquidityShare(pool_id, provider.clone());
    let current_shares: i128 = env.storage().persistent().get(&share_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&share_key, &(current_shares + shares));

    shares
}

/// Remove liquidity from a pool
pub fn remove_liquidity(
    env: &Env,
    pool_id: u64,
    provider: &Address,
    shares: i128,
) -> (i128, i128) {
    provider.require_auth();

    if shares <= 0 {
        panic!("Shares must be positive");
    }

    let pool_key = DataKey::Pool(pool_id);
    let mut pool: LiquidityPool = env
        .storage()
        .persistent()
        .get(&pool_key)
        .expect("Pool not found");

    // Check user has enough shares
    let share_key = DataKey::LiquidityShare(pool_id, provider.clone());
    let user_shares: i128 = env.storage().persistent().get(&share_key).unwrap_or(0);

    if user_shares < shares {
        panic!("Insufficient shares");
    }

    // Calculate amounts to return
    let amount_a = shares * pool.reserve_a / pool.total_shares;
    let amount_b = shares * pool.reserve_b / pool.total_shares;

    if amount_a <= 0 || amount_b <= 0 {
        panic!("Invalid withdrawal amount");
    }

    // Transfer tokens back to provider
    let token_client_a = token::Client::new(env, &pool.token_a);
    let token_client_b = token::Client::new(env, &pool.token_b);
    let contract_address = env.current_contract_address();

    token_client_a.transfer(&contract_address, provider, &amount_a);
    token_client_b.transfer(&contract_address, provider, &amount_b);

    // Update pool reserves
    pool.reserve_a -= amount_a;
    pool.reserve_b -= amount_b;
    pool.total_shares -= shares;

    env.storage().persistent().set(&pool_key, &pool);

    // Update user shares
    env.storage()
        .persistent()
        .set(&share_key, &(user_shares - shares));

    (amount_a, amount_b)
}

/// Get pool reserves
pub fn get_reserves(env: &Env, pool_id: u64) -> (i128, i128) {
    let pool: LiquidityPool = env
        .storage()
        .persistent()
        .get(&DataKey::Pool(pool_id))
        .expect("Pool not found");

    (pool.reserve_a, pool.reserve_b)
}

/// Get pool total shares
pub fn get_total_shares(env: &Env, pool_id: u64) -> i128 {
    let pool: LiquidityPool = env
        .storage()
        .persistent()
        .get(&DataKey::Pool(pool_id))
        .expect("Pool not found");

    pool.total_shares
}

/// Get pool fee
pub fn get_pool_fee(env: &Env, pool_id: u64) -> u32 {
    let pool: LiquidityPool = env
        .storage()
        .persistent()
        .get(&DataKey::Pool(pool_id))
        .expect("Pool not found");

    pool.fee_bps
}
