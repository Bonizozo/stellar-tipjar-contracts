//! Swap operations for AMM

use super::{SwapResult, DataKey};
use soroban_sdk::{token, Address, Env};

/// Perform a token swap
pub fn swap(
    env: &Env,
    pool_id: u64,
    sender: &Address,
    token_in: &Address,
    amount_in: i128,
    min_amount_out: i128,
) -> SwapResult {
    sender.require_auth();

    if amount_in <= 0 {
        panic!("Amount in must be positive");
    }

    let pool_key = DataKey::Pool(pool_id);
    let mut pool: super::LiquidityPool = env
        .storage()
        .persistent()
        .get(&pool_key)
        .expect("Pool not found");

    // Determine which token is being swapped
    let (reserve_in, reserve_out, token_out) = if *token_in == pool.token_a {
        (pool.reserve_a, pool.reserve_b, pool.token_b.clone())
    } else if *token_in == pool.token_b {
        (pool.reserve_b, pool.reserve_a, pool.token_a.clone())
    } else {
        panic!("Token not in pool");
    };

    // Calculate output amount using constant product formula
    let amount_out = calculate_output(amount_in, reserve_in, reserve_out, pool.fee_bps);

    if amount_out < min_amount_out {
        panic!("Slippage too high");
    }

    // Transfer tokens from sender to pool
    let token_client_in = token::Client::new(env, token_in);
    let token_client_out = token::Client::new(env, &token_out);
    let contract_address = env.current_contract_address();

    token_client_in.transfer(sender, &contract_address, &amount_in);

    // Calculate fee
    let fee_amount = amount_in * pool.fee_bps as i128 / 10000;

    // Update pool reserves
    if *token_in == pool.token_a {
        pool.reserve_a += amount_in;
        pool.reserve_b -= amount_out;
    } else {
        pool.reserve_b += amount_in;
        pool.reserve_a -= amount_out;
    }

    env.storage().persistent().set(&pool_key, &pool);

    // Transfer tokens out to sender
    token_client_out.transfer(&contract_address, sender, &amount_out);

    SwapResult {
        amount_out,
        fee_amount,
        new_reserve_a: pool.reserve_a,
        new_reserve_b: pool.reserve_b,
    }
}

/// Calculate output amount using constant product formula (x * y = k)
pub fn calculate_output(
    amount_in: i128,
    reserve_in: i128,
    reserve_out: i128,
    fee_bps: u32,
) -> i128 {
    if amount_in <= 0 || reserve_in <= 0 || reserve_out <= 0 {
        return 0;
    }

    // Apply fee: amount_in_with_fee = amount_in * (10000 - fee_bps) / 10000
    let amount_in_with_fee = amount_in * (10000 - fee_bps as i128) / 10000;

    // Constant product formula: amount_out = (amount_in_with_fee * reserve_out) / (reserve_in + amount_in_with_fee)
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = reserve_in + amount_in_with_fee;

    numerator / denominator
}

/// Get expected output amount for a swap (view function)
pub fn get_amount_out(
    env: &Env,
    pool_id: u64,
    token_in: &Address,
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

    calculate_output(amount_in, reserve_in, reserve_out, pool.fee_bps)
}

/// Get price impact for a swap
pub fn get_price_impact(
    env: &Env,
    pool_id: u64,
    token_in: &Address,
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

    // Current price: reserve_out / reserve_in
    let current_price = reserve_out * 10000 / reserve_in;

    // New price after swap
    let amount_out = calculate_output(amount_in, reserve_in, reserve_out, pool.fee_bps);
    let new_reserve_in = reserve_in + amount_in;
    let new_reserve_out = reserve_out - amount_out;
    let new_price = new_reserve_out * 10000 / new_reserve_in;

    // Price impact in basis points
    if current_price > new_price {
        (current_price - new_price) * 10000 / current_price
    } else {
        (new_price - current_price) * 10000 / current_price
    }
}
