//! Swap operations using the constant-product formula (x·y = k).

use soroban_sdk::{panic_with_error, token, Address, Env};

use crate::TipJarError;

use super::{get_pool, pool::accrue_pool_fee, save_pool, SwapResult};

// ── Core swap ────────────────────────────────────────────────────────────────

/// Execute a swap: send `amount_in` of `token_in`, receive at least
/// `min_amount_out` of the other token.
///
/// Fee is deducted from `amount_in` before applying the constant-product
/// formula. The fee is accrued into the pool's LP reward accumulator.
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
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    let mut pool = get_pool(env, pool_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AmmPoolNotFound));

    if pool.reserve_a == 0 || pool.reserve_b == 0 {
        panic_with_error!(env, TipJarError::AmmInsufficientLiquidity);
    }

    let (reserve_in, reserve_out, is_a_in) = if *token_in == pool.token_a {
        (pool.reserve_a, pool.reserve_b, true)
    } else if *token_in == pool.token_b {
        (pool.reserve_b, pool.reserve_a, false)
    } else {
        panic_with_error!(env, TipJarError::AmmTokenNotInPool)
    };

    // Fee on input
    let fee_amount = amount_in * pool.fee_bps as i128 / 10_000;
    let amount_in_after_fee = amount_in - fee_amount;

    // Constant-product: amount_out = reserve_out * amount_in_after_fee / (reserve_in + amount_in_after_fee)
    let amount_out = calculate_output(amount_in_after_fee, reserve_in, reserve_out);

    if amount_out <= 0 {
        panic_with_error!(env, TipJarError::AmmInsufficientLiquidity);
    }
    if amount_out < min_amount_out {
        panic_with_error!(env, TipJarError::AmmSlippageExceeded);
    }

    // Price impact in bps: (spot_price_before - effective_price) / spot_price_before
    let price_impact_bps = {
        // spot = reserve_out / reserve_in (× 10_000 for bps)
        let spot = reserve_out * 10_000 / reserve_in;
        let effective = amount_out * 10_000 / amount_in;
        if spot > effective {
            (spot - effective) * 10_000 / spot
        } else {
            0
        }
    };

    // Transfer token_in from sender to contract
    let contract = env.current_contract_address();
    token::Client::new(env, token_in).transfer(sender, &contract, &amount_in);

    // Update reserves
    if is_a_in {
        pool.reserve_a += amount_in;
        pool.reserve_b -= amount_out;
        // Accrue fee (denominated in token A)
        accrue_pool_fee(env, &mut pool, fee_amount);
    } else {
        pool.reserve_b += amount_in;
        pool.reserve_a -= amount_out;
        // Fee is in token B; convert to token A equivalent for accumulator
        // Use current ratio: fee_a_equiv = fee_b * reserve_a / reserve_b
        let fee_a_equiv = if pool.reserve_b > 0 {
            fee_amount * pool.reserve_a / pool.reserve_b
        } else {
            0
        };
        accrue_pool_fee(env, &mut pool, fee_a_equiv);
    }

    let (new_reserve_a, new_reserve_b) = (pool.reserve_a, pool.reserve_b);
    save_pool(env, &pool);

    // Transfer token_out to sender
    let token_out = if is_a_in {
        pool.token_b.clone()
    } else {
        pool.token_a.clone()
    };
    token::Client::new(env, &token_out).transfer(&contract, sender, &amount_out);

    SwapResult {
        amount_out,
        fee_amount,
        new_reserve_a,
        new_reserve_b,
        price_impact_bps,
    }
}

// ── Pure calculation helpers (no state) ─────────────────────────────────────

/// Constant-product output: `amount_out = reserve_out * x / (reserve_in + x)`
/// where `x = amount_in_after_fee`.
pub fn calculate_output(amount_in_after_fee: i128, reserve_in: i128, reserve_out: i128) -> i128 {
    if amount_in_after_fee <= 0 || reserve_in <= 0 || reserve_out <= 0 {
        return 0;
    }
    let numerator = amount_in_after_fee * reserve_out;
    let denominator = reserve_in + amount_in_after_fee;
    numerator / denominator
}

/// Reverse: given a desired `amount_out`, compute the required `amount_in`
/// (before fee deduction).
pub fn calculate_input_for_output(
    amount_out: i128,
    reserve_in: i128,
    reserve_out: i128,
    fee_bps: u32,
) -> i128 {
    if amount_out <= 0 || amount_out >= reserve_out || reserve_in <= 0 {
        return 0;
    }
    // amount_in_after_fee = reserve_in * amount_out / (reserve_out - amount_out)
    let numerator = reserve_in * amount_out;
    let denominator = reserve_out - amount_out;
    let amount_in_after_fee = numerator / denominator + 1; // ceil
                                                           // amount_in = amount_in_after_fee * 10_000 / (10_000 - fee_bps)
    amount_in_after_fee * 10_000 / (10_000 - fee_bps as i128) + 1
}

/// Price impact in basis points for a hypothetical swap.
pub fn price_impact_bps(
    amount_in: i128,
    reserve_in: i128,
    reserve_out: i128,
    fee_bps: u32,
) -> i128 {
    if reserve_in == 0 || reserve_out == 0 || amount_in <= 0 {
        return 0;
    }
    let fee = amount_in * fee_bps as i128 / 10_000;
    let amount_in_after_fee = amount_in - fee;
    let amount_out = calculate_output(amount_in_after_fee, reserve_in, reserve_out);
    if amount_out == 0 {
        return 10_000;
    }
    let spot = reserve_out * 10_000 / reserve_in;
    let effective = amount_out * 10_000 / amount_in;
    if spot > effective {
        (spot - effective) * 10_000 / spot
    } else {
        0
    }
}

/// View: expected output for a swap (no state change).
pub fn get_amount_out(env: &Env, pool_id: u64, token_in: &Address, amount_in: i128) -> i128 {
    let pool = get_pool(env, pool_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AmmPoolNotFound));

    let (reserve_in, reserve_out) = if *token_in == pool.token_a {
        (pool.reserve_a, pool.reserve_b)
    } else if *token_in == pool.token_b {
        (pool.reserve_b, pool.reserve_a)
    } else {
        panic_with_error!(env, TipJarError::AmmTokenNotInPool)
    };

    let fee = amount_in * pool.fee_bps as i128 / 10_000;
    calculate_output(amount_in - fee, reserve_in, reserve_out)
}

/// View: required input for a desired output (no state change).
pub fn get_amount_in(env: &Env, pool_id: u64, token_in: &Address, amount_out: i128) -> i128 {
    let pool = get_pool(env, pool_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AmmPoolNotFound));

    let (reserve_in, reserve_out) = if *token_in == pool.token_a {
        (pool.reserve_a, pool.reserve_b)
    } else if *token_in == pool.token_b {
        (pool.reserve_b, pool.reserve_a)
    } else {
        panic_with_error!(env, TipJarError::AmmTokenNotInPool)
    };

    calculate_input_for_output(amount_out, reserve_in, reserve_out, pool.fee_bps)
}
