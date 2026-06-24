//! Farming pool operations.

use soroban_sdk::{panic_with_error, token, Address, Env};

use crate::{DataKey, TipJarError, CoreError};

use super::{rewards, FarmingPool, FarmingPosition};

/// Creates a new farming pool and returns its pool ID.
pub fn create_pool(
    env: &Env,
    lp_token: &Address,
    reward_token: &Address,
    reward_rate_bps: u32,
    lock_period: u64,
) -> u64 {
    let pool_id = next_pool_id(env);
    let pool = FarmingPool {
        id: pool_id,
        lp_token: lp_token.clone(),
        reward_token: reward_token.clone(),
        reward_rate_bps,
        lock_period,
        total_staked: 0,
        created_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&DataKey::FarmingPool(pool_id), &pool);

    pool_id
}

/// Stakes LP tokens into a pool.
pub fn stake(env: &Env, staker: &Address, pool_id: u64, amount: i128) {
    if amount <= 0 {
        panic_with_error!(env, CoreError::InvalidAmount);
    }

    let mut pool = get_pool_or_panic(env, pool_id);
    let mut position = get_position(env, pool_id, staker).unwrap_or(FarmingPosition {
        staker: staker.clone(),
        amount: 0,
        last_update: env.ledger().timestamp(),
        stake_timestamp: env.ledger().timestamp(),
        pending_rewards: 0,
    });

    rewards::accrue_rewards(&pool, &mut position, env.ledger().timestamp());

    let lp_client = token::Client::new(env, &pool.lp_token);
    lp_client.transfer(staker, &env.current_contract_address(), &amount);

    position.amount += amount;
    position.stake_timestamp = env.ledger().timestamp();

    pool.total_staked += amount;
    set_pool(env, &pool);
    set_position(env, pool_id, &position);
}

/// Harvests accrued rewards from a pool.
pub fn harvest(env: &Env, staker: &Address, pool_id: u64) -> i128 {
    let pool = get_pool_or_panic(env, pool_id);
    let mut position = get_position_or_panic(env, pool_id, staker);

    rewards::accrue_rewards(&pool, &mut position, env.ledger().timestamp());

    let rewards_due = position.pending_rewards;
    if rewards_due > 0 {
        let reward_client = token::Client::new(env, &pool.reward_token);
        reward_client.transfer(&env.current_contract_address(), staker, &rewards_due);
        position.pending_rewards = 0;
        set_position(env, pool_id, &position);
    }

    rewards_due
}

/// Unstakes LP tokens from a pool after lock period.
pub fn unstake(env: &Env, staker: &Address, pool_id: u64, amount: i128) {
    if amount <= 0 {
        panic_with_error!(env, CoreError::InvalidAmount);
    }

    let mut pool = get_pool_or_panic(env, pool_id);
    let mut position = get_position_or_panic(env, pool_id, staker);

    if amount > position.amount {
        panic_with_error!(env, CoreError::InsufficientBalance);
    }

    let now = env.ledger().timestamp();
    if !rewards::lock_expired(position.stake_timestamp, pool.lock_period, now) {
        panic_with_error!(env, TipJarError::FarmingLockNotExpired);
    }

    rewards::accrue_rewards(&pool, &mut position, now);

    position.amount -= amount;
    pool.total_staked -= amount;

    let lp_client = token::Client::new(env, &pool.lp_token);
    lp_client.transfer(&env.current_contract_address(), staker, &amount);

    set_pool(env, &pool);
    set_position(env, pool_id, &position);
}

/// Gets a farming pool by ID.
pub fn get_pool(env: &Env, pool_id: u64) -> Option<FarmingPool> {
    env.storage().persistent().get(&DataKey::FarmingPool(pool_id))
}

/// Gets a farming pool or panics with contract error.
pub fn get_pool_or_panic(env: &Env, pool_id: u64) -> FarmingPool {
    get_pool(env, pool_id).unwrap_or_else(|| panic_with_error!(env, TipJarError::FarmingPoolNotFound))
}

/// Gets a farming position for `(pool_id, staker)`.
pub fn get_position(env: &Env, pool_id: u64, staker: &Address) -> Option<FarmingPosition> {
    env.storage()
        .persistent()
        .get(&DataKey::FarmingPosition(pool_id, staker.clone()))
}

fn get_position_or_panic(env: &Env, pool_id: u64, staker: &Address) -> FarmingPosition {
    get_position(env, pool_id, staker)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::FarmingPositionNotFound))
}

fn set_pool(env: &Env, pool: &FarmingPool) {
    env.storage()
        .persistent()
        .set(&DataKey::FarmingPool(pool.id), pool);
}

fn set_position(env: &Env, pool_id: u64, position: &FarmingPosition) {
    env.storage()
        .persistent()
        .set(&DataKey::FarmingPosition(pool_id, position.staker.clone()), position);
}

fn next_pool_id(env: &Env) -> u64 {
    let current = env
        .storage()
        .persistent()
        .get::<_, u64>(&DataKey::FarmingPoolCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .persistent()
        .set(&DataKey::FarmingPoolCounter, &next);
    next
}




