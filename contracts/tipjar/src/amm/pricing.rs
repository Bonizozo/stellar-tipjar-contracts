//! Price calculation utilities for AMM

use super::DataKey;
use soroban_sdk::Env;

/// Get current price for token A in terms of token B
pub fn get_price(
    env: &Env,
    pool_id: u64,
    token_a: &soroban_sdk::Address,
) -> i128 {
    let pool: super::LiquidityPool = env
        .storage()
        .persistent()
        .get(&DataKey::Pool(pool_id))
        .expect("Pool not found");

    if *token_a == pool.token_a {
        // Price of token A in terms of token B
        if pool.reserve_a == 0 {
            return 0;
        }
        pool.reserve_b * 10000 / pool.reserve_a
    } else if *token_a == pool.token_b {
        // Price of token B in terms of token A
        if pool.reserve_b == 0 {
            return 0;
        }
        pool.reserve_a * 10000 / pool.reserve_b
    } else {
        panic!("Token not in pool");
    }
}

/// Get pool liquidity value
pub fn get_liquidity_value(
    env: &Env,
    pool_id: u64,
) -> (i128, i128) {
    let pool: super::LiquidityPool = env
        .storage()
        .persistent()
        .get(&DataKey::Pool(pool_id))
        .expect("Pool not found");

    (pool.reserve_a, pool.reserve_b)
}

/// Get pool utilization rate
pub fn get_utilization_rate(
    env: &Env,
    pool_id: u64,
) -> i128 {
    let pool: super::LiquidityPool = env
        .storage()
        .persistent()
        .get(&DataKey::Pool(pool_id))
        .expect("Pool not found");

    if pool.total_shares == 0 {
        return 0;
    }

    // Utilization = (reserve_a + reserve_b) / total_shares * 10000
    let total_value = pool.reserve_a + pool.reserve_b;
    total_value * 10000 / pool.total_shares
}

/// Calculate optimal input amount for desired output
pub fn calculate_optimal_input(
    env: &Env,
    pool_id: u64,
    token_in: &soroban_sdk::Address,
    desired_output: i128,
) -> i128 {
    let pool: super::LiquidityPool = env
        .storage()
        .persistent()
        .get(&DataKey::Pool(pool_id))
        .expect("Pool not found");

    let (reserve_in, reserve_out) = if *token_in == pool.token_a {
        (pool.reserve_a, pool.reserve_b)
    } else if *token_in == pool.token_b {
        (pool.reserve_b, pool.reserve_a)
    } else {
        panic!("Token not in pool");
    };

    if desired_output <= 0 || desired_output >= reserve_out {
        return 0;
    }

    // Reverse calculation: amount_in = (reserve_in * desired_output) / (reserve_out - desired_output)
    // Adjusted for fee
    let numerator = reserve_in * desired_output * 10000;
    let denominator = (reserve_out - desired_output) * (10000 - pool.fee_bps as i128);

    numerator / denominator + 1 // Add 1 to ensure we get at least the desired output
}

/// Calculate optimal output amount for given input
pub fn calculate_optimal_output(
    env: &Env,
    pool_id: u64,
    token_in: &soroban_sdk::Address,
    amount_in: i128,
) -> i128 {
    let pool: super::LiquidityPool = env
        .storage()
        .persistent()
        .get(&DataKey::Pool(pool_id))
        .expect("Pool not found");

    let (reserve_in, reserve_out) = if *token_in == pool.token_a {
        (pool.reserve_a, pool.reserve_b)
    } else if *token_in == pool.token_b {
        (pool.reserve_b, pool.reserve_a)
    } else {
        panic!("Token not in pool");
    };

    super::swap::calculate_output(amount_in, reserve_in, reserve_out, pool.fee_bps)
}

/// Get mid price (average of buy and sell prices)
pub fn get_mid_price(
    env: &Env,
    pool_id: u64,
    token_a: &soroban_sdk::Address,
) -> i128 {
    let pool: super::LiquidityPool = env
        .storage()
        .persistent()
        .get(&DataKey::Pool(pool_id))
        .expect("Pool not found");

    if pool.reserve_a == 0 || pool.reserve_b == 0 {
        return 0;
    }

    if *token_a == pool.token_a {
        // Mid price = (reserve_b / reserve_a) * 10000
        pool.reserve_b * 10000 / pool.reserve_a
    } else if *token_a == pool.token_b {
        // Mid price = (reserve_a / reserve_b) * 10000
        pool.reserve_a * 10000 / pool.reserve_b
    } else {
        panic!("Token not in pool");
    }
}
