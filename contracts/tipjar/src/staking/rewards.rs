//! Rewards calculation for staking

use super::StakeInfo;
use soroban_sdk::Env;

/// Calculate pending rewards for a staker
pub fn calculate_pending_rewards(env: &Env, stake_info: &StakeInfo) -> i128 {
    let now = env.ledger().timestamp();
    let time_staked = now - stake_info.last_claim;

    if time_staked == 0 {
        return 0;
    }

    let total_staked = super::get_total_staked(env);
    if total_staked == 0 {
        return 0;
    }

    let config = super::get_staking_config(env);

    // Calculate base rewards
    // reward = (stake_amount / total_staked) * reward_pool * (time_staked / year)
    let year_seconds = 365 * 24 * 3600;
    let time_fraction = time_staked * 1_000_000 / year_seconds; // Use 1_000_000 as base

    let share = stake_info.amount * 1_000_000 / total_staked;
    let base_rewards = config.reward_pool * share / 1_000_000 * time_fraction / 1_000_000;

    // Apply time multiplier
    let time_multiplier = calculate_time_multiplier(env, stake_info);
    base_rewards * time_multiplier / 1_000_000
}

/// Calculate time multiplier based on staking duration
pub fn calculate_time_multiplier(env: &Env, stake_info: &StakeInfo) -> i128 {
    let now = env.ledger().timestamp();
    let total_time = now - stake_info.stake_time;

    let config = super::get_staking_config(env);

    if total_time >= config.time_weight_period {
        // Maximum multiplier reached
        config.max_time_multiplier
    } else {
        // Linear interpolation: multiplier = (total_time / time_weight_period) * max_multiplier
        total_time * config.max_time_multiplier / config.time_weight_period
    }
}

/// Calculate total rewards for a staker (accumulated + pending)
pub fn calculate_total_rewards(env: &Env, stake_info: &StakeInfo) -> i128 {
    let pending = calculate_pending_rewards(env, stake_info);
    stake_info.accumulated_rewards + pending
}

/// Calculate reward rate for a staker
pub fn calculate_reward_rate(env: &Env, stake_info: &StakeInfo) -> i128 {
    let total_staked = super::get_total_staked(env);
    if total_staked == 0 {
        return 0;
    }

    let config = super::get_staking_config(env);

    // Annual reward rate = (stake_amount / total_staked) * reward_rate_bps
    stake_info.amount * config.reward_rate_bps as i128 / total_staked
}

/// Calculate impermanent loss (for AMM integration)
pub fn calculate_impermanent_loss(
    price_change_ratio: i128, // Price change ratio * 1_000_000
) -> i128 {
    // Simplified impermanent loss calculation
    // IL = 2 * sqrt(price_ratio) / (1 + price_ratio) - 1
    // For small price changes, IL ≈ 0.5 * (price_change)^2

    if price_change_ratio == 1_000_000 {
        // No price change
        return 0;
    }

    // Use approximation for small changes
    let price_change = if price_change_ratio > 1_000_000 {
        price_change_ratio - 1_000_000
    } else {
        1_000_000 - price_change_ratio
    };

    // IL ≈ 0.5 * (price_change / 1_000_000)^2 * 1_000_000
    let il = price_change * price_change / 2_000_000;
    il
}

/// Calculate expected rewards for a given stake amount and duration
pub fn calculate_expected_rewards(
    env: &Env,
    stake_amount: i128,
    duration: u64,
) -> i128 {
    let total_staked = super::get_total_staked(env);
    if total_staked == 0 {
        return 0;
    }

    let config = super::get_staking_config(env);

    // Base rewards
    let year_seconds = 365 * 24 * 3600;
    let time_fraction = duration * 1_000_000 / year_seconds;

    let share = stake_amount * 1_000_000 / total_staked;
    let base_rewards = config.reward_pool * share / 1_000_000 * time_fraction / 1_000_000;

    // Time multiplier
    let time_multiplier = if duration >= config.time_weight_period {
        config.max_time_multiplier
    } else {
        duration * config.max_time_multiplier / config.time_weight_period
    };

    base_rewards * time_multiplier / 1_000_000
}
