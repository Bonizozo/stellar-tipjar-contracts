//! Price and pool analytics for the AMM.

use soroban_sdk::{panic_with_error, Address, Env};

use crate::TipJarError;

use super::{get_pool, swap::price_impact_bps};

/// Spot price of `token_in` in terms of the other token, scaled × 1_000_000.
///
/// Returns `reserve_out / reserve_in × 1_000_000`.
pub fn spot_price(env: &Env, pool_id: u64, token_in: &Address) -> i128 {
    let pool = get_pool(env, pool_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AmmPoolNotFound));

    let (reserve_in, reserve_out) = if *token_in == pool.token_a {
        (pool.reserve_a, pool.reserve_b)
    } else if *token_in == pool.token_b {
        (pool.reserve_b, pool.reserve_a)
    } else {
        panic_with_error!(env, TipJarError::AmmTokenNotInPool)
    };

    if reserve_in == 0 {
        return 0;
    }
    reserve_out * 1_000_000 / reserve_in
}

/// Price impact in basis points for a hypothetical swap of `amount_in`.
pub fn get_price_impact(env: &Env, pool_id: u64, token_in: &Address, amount_in: i128) -> i128 {
    let pool = get_pool(env, pool_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AmmPoolNotFound));

    let (reserve_in, reserve_out) = if *token_in == pool.token_a {
        (pool.reserve_a, pool.reserve_b)
    } else if *token_in == pool.token_b {
        (pool.reserve_b, pool.reserve_a)
    } else {
        panic_with_error!(env, TipJarError::AmmTokenNotInPool)
    };

    price_impact_bps(amount_in, reserve_in, reserve_out, pool.fee_bps)
}

/// Constant-product invariant k = reserve_a × reserve_b.
pub fn get_invariant(env: &Env, pool_id: u64) -> i128 {
    let pool = get_pool(env, pool_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AmmPoolNotFound));
    pool.reserve_a * pool.reserve_b
}

/// Total value locked expressed as `(reserve_a, reserve_b)`.
pub fn get_tvl(env: &Env, pool_id: u64) -> (i128, i128) {
    let pool = get_pool(env, pool_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AmmPoolNotFound));
    (pool.reserve_a, pool.reserve_b)
}

/// LP share value: how much of each token one share is worth.
/// Returns `(token_a_per_share × 1_000_000, token_b_per_share × 1_000_000)`.
pub fn share_value(env: &Env, pool_id: u64) -> (i128, i128) {
    let pool = get_pool(env, pool_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AmmPoolNotFound));
    if pool.total_shares == 0 {
        return (0, 0);
    }
    (
        pool.reserve_a * 1_000_000 / pool.total_shares,
        pool.reserve_b * 1_000_000 / pool.total_shares,
    )
}

/// Total fees collected by the pool since creation.
pub fn total_fees_collected(env: &Env, pool_id: u64) -> i128 {
    let pool = get_pool(env, pool_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AmmPoolNotFound));
    pool.total_fees_collected
}
