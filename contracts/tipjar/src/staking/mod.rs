//! Staking and Rewards Distribution
//!
//! This module provides staking functionality with time-weighted rewards.

pub mod rewards;
pub mod distribution;

use soroban_sdk::{contracttype, token, Address, Env};

/// Stake information for a user
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StakeInfo {
    pub amount: i128,
    pub stake_time: u64,
    pub last_claim: u64,
    pub accumulated_rewards: i128,
}

/// Staking configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StakingConfig {
    pub reward_pool: i128,
    pub total_staked: i128,
    pub reward_rate_bps: u32, // Reward rate in basis points per year
    pub unstake_cooldown: u64, // Cooldown period in seconds
    pub max_time_multiplier: i128, // Maximum time multiplier (e.g., 2_000_000 for 2x)
    pub time_weight_period: u64, // Period for time weighting in seconds
}

/// Storage keys for staking
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    StakeInfo(Address),
    StakingConfig,
    RewardPool,
    TotalStaked,
    StakedToken,
}

/// Default staking constants
pub const DEFAULT_REWARD_RATE_BPS: u32 = 1000; // 10% per year
pub const DEFAULT_UNSTAKE_COOLDOWN: u64 = 604800; // 7 days
pub const DEFAULT_MAX_TIME_MULTIPLIER: i128 = 2_000_000; // 2x after time_weight_period
pub const DEFAULT_TIME_WEIGHT_PERIOD: u64 = 2592000; // 30 days

/// Initialize staking configuration
pub fn init_staking(env: &Env, staked_token: &Address) {
    let config = StakingConfig {
        reward_pool: 0,
        total_staked: 0,
        reward_rate_bps: DEFAULT_REWARD_RATE_BPS,
        unstake_cooldown: DEFAULT_UNSTAKE_COOLDOWN,
        max_time_multiplier: DEFAULT_MAX_TIME_MULTIPLIER,
        time_weight_period: DEFAULT_TIME_WEIGHT_PERIOD,
    };

    env.storage()
        .persistent()
        .set(&DataKey::StakingConfig, &config);
    env.storage()
        .persistent()
        .set(&DataKey::StakedToken, staked_token);
}

/// Get staking configuration
pub fn get_staking_config(env: &Env) -> StakingConfig {
    env.storage()
        .persistent()
        .get(&DataKey::StakingConfig)
        .expect("Staking not initialized")
}

/// Update staking configuration
pub fn update_staking_config(env: &Env, config: &StakingConfig) {
    env.storage()
        .persistent()
        .set(&DataKey::StakingConfig, config);
}

/// Get stake info for a user
pub fn get_stake_info(env: &Env, staker: &Address) -> Option<StakeInfo> {
    env.storage()
        .persistent()
        .get(&DataKey::StakeInfo(staker.clone()))
}

/// Get stake info or panic if not found
pub fn get_stake_info_or_panic(env: &Env, staker: &Address) -> StakeInfo {
    get_stake_info(env, staker).expect("No stake found")
}

/// Update stake info for a user
pub fn update_stake_info(env: &Env, staker: &Address, stake_info: &StakeInfo) {
    env.storage()
        .persistent()
        .set(&DataKey::StakeInfo(staker.clone()), stake_info);
}

/// Get total staked amount
pub fn get_total_staked(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::TotalStaked)
        .unwrap_or(0)
}

/// Update total staked amount
pub fn update_total_staked(env: &Env, amount: i128) {
    env.storage().persistent().set(&DataKey::TotalStaked, &amount);
}

/// Get reward pool balance
pub fn get_reward_pool(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::RewardPool)
        .unwrap_or(0)
}

/// Update reward pool balance
pub fn update_reward_pool(env: &Env, amount: i128) {
    env.storage().persistent().set(&DataKey::RewardPool, &amount);
}

/// Get staked token address
pub fn get_staked_token(env: &Env) -> Address {
    env.storage()
        .persistent()
        .get(&DataKey::StakedToken)
        .expect("Staking not initialized")
}

/// Add rewards to the pool
pub fn add_rewards(env: &Env, amount: i128) {
    if amount <= 0 {
        panic!("Reward amount must be positive");
    }

    let current_pool = get_reward_pool(env);
    update_reward_pool(env, current_pool + amount);

    let mut config = get_staking_config(env);
    config.reward_pool += amount;
    update_staking_config(env, &config);
}
