//! Staking distribution logic

use super::StakeInfo;
use soroban_sdk::{token, Address, Env};

/// Stake tokens
pub fn stake(env: &Env, staker: &Address, amount: i128) {
    staker.require_auth();

    if amount <= 0 {
        panic!("Stake amount must be positive");
    }

    let staked_token = super::get_staked_token(env);
    let token_client = token::Client::new(env, &staked_token);
    let contract_address = env.current_contract_address();

    // Transfer tokens from staker to contract
    token_client.transfer(staker, &contract_address, &amount);

    // Get or create stake info
    let mut stake_info = super::get_stake_info(env, staker).unwrap_or(StakeInfo {
        amount: 0,
        stake_time: env.ledger().timestamp(),
        last_claim: env.ledger().timestamp(),
        accumulated_rewards: 0,
    });

    // Claim pending rewards before updating stake
    let pending = super::rewards::calculate_pending_rewards(env, &stake_info);
    stake_info.accumulated_rewards += pending;

    // Update stake
    stake_info.amount += amount;
    stake_info.last_claim = env.ledger().timestamp();

    super::update_stake_info(env, staker, &stake_info);

    // Update total staked
    let total_staked = super::get_total_staked(env);
    super::update_total_staked(env, total_staked + amount);

    // Emit event
    env.events().publish(
        (soroban_sdk::symbol_short!("stake"),),
        (staker.clone(), amount, stake_info.amount),
    );
}

/// Claim rewards
pub fn claim_rewards(env: &Env, staker: &Address) -> i128 {
    staker.require_auth();

    let mut stake_info = super::get_stake_info_or_panic(env, staker);

    let pending = super::rewards::calculate_pending_rewards(env, &stake_info);
    let total_rewards = stake_info.accumulated_rewards + pending;

    if total_rewards <= 0 {
        panic!("No rewards to claim");
    }

    // Transfer rewards
    let staked_token = super::get_staked_token(env);
    let token_client = token::Client::new(env, &staked_token);
    let contract_address = env.current_contract_address();

    token_client.transfer(&contract_address, staker, &total_rewards);

    // Update reward pool
    let reward_pool = super::get_reward_pool(env);
    super::update_reward_pool(env, reward_pool - total_rewards);

    // Reset accumulated rewards
    stake_info.accumulated_rewards = 0;
    stake_info.last_claim = env.ledger().timestamp();

    super::update_stake_info(env, staker, &stake_info);

    // Emit event
    env.events().publish(
        (soroban_sdk::symbol_short!("claim"),),
        (staker.clone(), total_rewards),
    );

    total_rewards
}

/// Unstake tokens
pub fn unstake(env: &Env, staker: &Address, amount: i128) {
    staker.require_auth();

    if amount <= 0 {
        panic!("Unstake amount must be positive");
    }

    let mut stake_info = super::get_stake_info_or_panic(env, staker);

    // Check cooldown period
    let now = env.ledger().timestamp();
    let config = super::get_staking_config(env);
    if now < stake_info.stake_time + config.unstake_cooldown {
        panic!("Cooldown period not expired");
    }

    if amount > stake_info.amount {
        panic!("Insufficient staked amount");
    }

    // Claim rewards first
    claim_rewards(env.clone(), staker.clone());

    // Update stake
    stake_info.amount -= amount;
    super::update_stake_info(env, staker, &stake_info);

    // Update total staked
    let total_staked = super::get_total_staked(env);
    super::update_total_staked(env, total_staked - amount);

    // Transfer tokens back
    let staked_token = super::get_staked_token(env);
    let token_client = token::Client::new(env, &staked_token);
    let contract_address = env.current_contract_address();

    token_client.transfer(&contract_address, staker, &amount);

    // Emit event
    env.events().publish(
        (soroban_sdk::symbol_short!("unstake"),),
        (staker.clone(), amount, stake_info.amount),
    );
}

/// Get pending rewards for a staker
pub fn get_pending_rewards(env: &Env, staker: &Address) -> i128 {
    let stake_info = super::get_stake_info(env, staker);
    match stake_info {
        Some(info) => super::rewards::calculate_pending_rewards(env, &info),
        None => 0,
    }
}

/// Get total rewards for a staker
pub fn get_total_rewards(env: &Env, staker: &Address) -> i128 {
    let stake_info = super::get_stake_info(env, staker);
    match stake_info {
        Some(info) => super::rewards::calculate_total_rewards(env, &info),
        None => 0,
    }
}

/// Get staker's current stake amount
pub fn get_stake_amount(env: &Env, staker: &Address) -> i128 {
    let stake_info = super::get_stake_info(env, staker);
    match stake_info {
        Some(info) => info.amount,
        None => 0,
    }
}

/// Check if staker can unstake
pub fn can_unstake(env: &Env, staker: &Address) -> bool {
    let stake_info = super::get_stake_info(env, staker);
    match stake_info {
        Some(info) => {
            let now = env.ledger().timestamp();
            let config = super::get_staking_config(env);
            now >= info.stake_time + config.unstake_cooldown
        }
        None => false,
    }
}

/// Get unstake cooldown remaining
pub fn get_unstake_cooldown_remaining(env: &Env, staker: &Address) -> u64 {
    let stake_info = super::get_stake_info(env, staker);
    match stake_info {
        Some(info) => {
            let now = env.ledger().timestamp();
            let config = super::get_staking_config(env);
            let cooldown_end = info.stake_time + config.unstake_cooldown;
            if now >= cooldown_end {
                0
            } else {
                cooldown_end - now
            }
        }
        None => 0,
    }
}
