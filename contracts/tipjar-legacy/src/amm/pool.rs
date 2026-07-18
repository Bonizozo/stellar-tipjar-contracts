//! Liquidity pool operations: add / remove liquidity, LP share management.

use soroban_sdk::{panic_with_error, token, Address, Env};

use crate::TipJarError;

use super::{
    accrue_fee, get_lp_shares, get_pool, get_pool_id_by_tokens, isqrt, next_pool_id,
    pending_rewards, register_pool_tokens, save_pool, set_lp_shares, set_provider_debt,
    settle_rewards, AddLiquidityResult, LiquidityPool, RemoveLiquidityResult, DEFAULT_FEE_BPS,
    MIN_INITIAL_LIQUIDITY,
};

// ── Pool creation ────────────────────────────────────────────────────────────

/// Create a new liquidity pool for a token pair and seed it with initial
/// liquidity. The creator receives the initial LP shares.
///
/// Returns `(pool_id, shares_minted)`.
pub fn create_pool(
    env: &Env,
    creator: &Address,
    token_a: &Address,
    token_b: &Address,
    amount_a: i128,
    amount_b: i128,
    fee_bps: Option<u32>,
) -> (u64, i128) {
    creator.require_auth();

    if token_a == token_b {
        panic_with_error!(env, TipJarError::AmmIdenticalTokens);
    }
    if amount_a < MIN_INITIAL_LIQUIDITY || amount_b < MIN_INITIAL_LIQUIDITY {
        panic_with_error!(env, TipJarError::AmmInsufficientLiquidity);
    }

    let effective_fee = fee_bps.unwrap_or(DEFAULT_FEE_BPS);
    if effective_fee > super::MAX_FEE_BPS {
        panic_with_error!(env, TipJarError::AmmFeeTooHigh);
    }

    // Reject duplicate pools
    if get_pool_id_by_tokens(env, token_a, token_b).is_some() {
        panic_with_error!(env, TipJarError::AmmPoolExists);
    }

    let pool_id = next_pool_id(env);

    // Initial shares = geometric mean of deposits (Uniswap v2 style)
    let shares = isqrt(amount_a * amount_b);
    if shares <= 0 {
        panic_with_error!(env, TipJarError::AmmInsufficientLiquidity);
    }

    // Transfer tokens from creator to contract
    let contract = env.current_contract_address();
    token::Client::new(env, token_a).transfer(creator, &contract, &amount_a);
    token::Client::new(env, token_b).transfer(creator, &contract, &amount_b);

    let pool = LiquidityPool {
        pool_id,
        token_a: token_a.clone(),
        token_b: token_b.clone(),
        reserve_a: amount_a,
        reserve_b: amount_b,
        total_shares: shares,
        fee_bps: effective_fee,
        fee_per_share_accum: 0,
        total_fees_collected: 0,
    };

    save_pool(env, &pool);
    register_pool_tokens(env, pool_id, token_a, token_b);

    set_lp_shares(env, pool_id, creator, shares);
    set_provider_debt(env, pool_id, creator, 0);

    (pool_id, shares)
}

// ── Add liquidity ────────────────────────────────────────────────────────────

/// Add liquidity to an existing pool.
///
/// The actual amounts deposited are adjusted to maintain the current pool
/// ratio. The caller specifies `amount_a_desired` and `amount_b_desired`
/// plus minimum acceptable amounts for slippage protection.
pub fn add_liquidity(
    env: &Env,
    pool_id: u64,
    provider: &Address,
    amount_a_desired: i128,
    amount_b_desired: i128,
    amount_a_min: i128,
    amount_b_min: i128,
) -> AddLiquidityResult {
    provider.require_auth();

    if amount_a_desired <= 0 || amount_b_desired <= 0 {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    // Settle any pending rewards before changing share balance
    settle_rewards(env, pool_id, provider);

    let mut pool = get_pool(env, pool_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AmmPoolNotFound));

    // Calculate optimal deposit amounts to maintain ratio
    let (amount_a, amount_b) = if pool.total_shares == 0 {
        (amount_a_desired, amount_b_desired)
    } else {
        let optimal_b = amount_a_desired * pool.reserve_b / pool.reserve_a;
        if optimal_b <= amount_b_desired {
            if optimal_b < amount_b_min {
                panic_with_error!(env, TipJarError::AmmSlippageExceeded);
            }
            (amount_a_desired, optimal_b)
        } else {
            let optimal_a = amount_b_desired * pool.reserve_a / pool.reserve_b;
            if optimal_a < amount_a_min {
                panic_with_error!(env, TipJarError::AmmSlippageExceeded);
            }
            (optimal_a, amount_b_desired)
        }
    };

    if amount_a < amount_a_min || amount_b < amount_b_min {
        panic_with_error!(env, TipJarError::AmmSlippageExceeded);
    }

    // Mint shares proportional to the smaller ratio
    let shares = if pool.total_shares == 0 {
        isqrt(amount_a * amount_b)
    } else {
        let s_a = amount_a * pool.total_shares / pool.reserve_a;
        let s_b = amount_b * pool.total_shares / pool.reserve_b;
        s_a.min(s_b)
    };

    if shares <= 0 {
        panic_with_error!(env, TipJarError::AmmInsufficientLiquidity);
    }

    // Transfer tokens
    let contract = env.current_contract_address();
    token::Client::new(env, &pool.token_a).transfer(provider, &contract, &amount_a);
    token::Client::new(env, &pool.token_b).transfer(provider, &contract, &amount_b);

    pool.reserve_a += amount_a;
    pool.reserve_b += amount_b;
    pool.total_shares += shares;
    save_pool(env, &pool);

    // Update LP shares and sync debt to current accumulator
    let current_shares = get_lp_shares(env, pool_id, provider);
    set_lp_shares(env, pool_id, provider, current_shares + shares);
    set_provider_debt(env, pool_id, provider, pool.fee_per_share_accum);

    AddLiquidityResult {
        shares_minted: shares,
        amount_a,
        amount_b,
    }
}

// ── Remove liquidity ─────────────────────────────────────────────────────────

/// Remove liquidity from a pool by burning LP shares.
///
/// Automatically claims any pending fee rewards.
/// `amount_a_min` / `amount_b_min` provide slippage protection.
pub fn remove_liquidity(
    env: &Env,
    pool_id: u64,
    provider: &Address,
    shares: i128,
    amount_a_min: i128,
    amount_b_min: i128,
) -> RemoveLiquidityResult {
    provider.require_auth();

    if shares <= 0 {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    let user_shares = get_lp_shares(env, pool_id, provider);
    if user_shares < shares {
        panic_with_error!(env, TipJarError::AmmInsufficientShares);
    }

    // Claim pending rewards before burning shares
    let rewards = settle_rewards(env, pool_id, provider);

    let mut pool = get_pool(env, pool_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AmmPoolNotFound));

    let amount_a = shares * pool.reserve_a / pool.total_shares;
    let amount_b = shares * pool.reserve_b / pool.total_shares;

    if amount_a < amount_a_min || amount_b < amount_b_min {
        panic_with_error!(env, TipJarError::AmmSlippageExceeded);
    }
    if amount_a <= 0 || amount_b <= 0 {
        panic_with_error!(env, TipJarError::AmmInsufficientLiquidity);
    }

    pool.reserve_a -= amount_a;
    pool.reserve_b -= amount_b;
    pool.total_shares -= shares;
    save_pool(env, &pool);

    set_lp_shares(env, pool_id, provider, user_shares - shares);

    // Transfer tokens back
    let contract = env.current_contract_address();
    token::Client::new(env, &pool.token_a).transfer(&contract, provider, &amount_a);
    token::Client::new(env, &pool.token_b).transfer(&contract, provider, &amount_b);

    // Transfer fee rewards (denominated in token A)
    if rewards > 0 {
        token::Client::new(env, &pool.token_a).transfer(&contract, provider, &rewards);
    }

    RemoveLiquidityResult {
        amount_a,
        amount_b,
        rewards_claimed: rewards,
    }
}

// ── Reward claim ─────────────────────────────────────────────────────────────

/// Claim accumulated fee rewards without removing liquidity.
/// Returns the amount of token A transferred to the provider.
pub fn claim_rewards(env: &Env, pool_id: u64, provider: &Address) -> i128 {
    provider.require_auth();

    let pool = get_pool(env, pool_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AmmPoolNotFound));

    let rewards = settle_rewards(env, pool_id, provider);
    if rewards == 0 {
        return 0;
    }

    let contract = env.current_contract_address();
    token::Client::new(env, &pool.token_a).transfer(&contract, provider, &rewards);

    rewards
}

// ── Queries ──────────────────────────────────────────────────────────────────

/// Return the LP share balance for a provider.
pub fn get_provider_shares(env: &Env, pool_id: u64, provider: &Address) -> i128 {
    get_lp_shares(env, pool_id, provider)
}

/// Return pending (unclaimed) fee rewards for a provider.
pub fn get_pending_rewards(env: &Env, pool_id: u64, provider: &Address) -> i128 {
    pending_rewards(env, pool_id, provider)
}

/// Return the pool's current reserves.
pub fn get_reserves(env: &Env, pool_id: u64) -> (i128, i128) {
    let pool = get_pool(env, pool_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AmmPoolNotFound));
    (pool.reserve_a, pool.reserve_b)
}

/// Update the fee for a pool. Admin only (enforced at contract level).
pub fn set_pool_fee(env: &Env, pool_id: u64, fee_bps: u32) {
    let mut pool = get_pool(env, pool_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::AmmPoolNotFound));
    if fee_bps > super::MAX_FEE_BPS {
        panic_with_error!(env, TipJarError::AmmFeeTooHigh);
    }
    pool.fee_bps = fee_bps;
    save_pool(env, &pool);
}

// ── Internal: accrue fee into pool (called from swap) ────────────────────────

pub(super) fn accrue_pool_fee(env: &Env, pool: &mut LiquidityPool, fee_amount: i128) {
    accrue_fee(env, pool, fee_amount);
}
