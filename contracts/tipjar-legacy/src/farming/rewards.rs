//! Reward and APY calculation helpers for farming.

use super::{FarmingPool, FarmingPosition};

/// Number of seconds in a year used for APY calculations.
pub const SECONDS_PER_YEAR: i128 = 31_536_000;

/// Returns APY in basis points for a pool.
pub fn calculate_apy_bps(pool: &FarmingPool) -> u32 {
    pool.reward_rate_bps
}

/// Computes rewards for `amount` over elapsed seconds.
pub fn calculate_rewards(amount: i128, reward_rate_bps: u32, elapsed_seconds: u64) -> i128 {
    if amount <= 0 || elapsed_seconds == 0 || reward_rate_bps == 0 {
        return 0;
    }

    amount
        * reward_rate_bps as i128
        * elapsed_seconds as i128
        / (10_000 * SECONDS_PER_YEAR)
}

/// Accrues rewards into a position up to `now` timestamp.
pub fn accrue_rewards(pool: &FarmingPool, position: &mut FarmingPosition, now: u64) {
    if now <= position.last_update {
        return;
    }

    let elapsed = now - position.last_update;
    let additional = calculate_rewards(position.amount, pool.reward_rate_bps, elapsed);
    position.pending_rewards += additional;
    position.last_update = now;
}

/// Returns true when unstake lock period has elapsed.
pub fn lock_expired(stake_timestamp: u64, lock_period: u64, now: u64) -> bool {
    now >= stake_timestamp + lock_period
}

#[cfg(test)]
mod tests {
    use soroban_sdk::Address;

    use super::{accrue_rewards, calculate_rewards, lock_expired, SECONDS_PER_YEAR};
    use crate::farming::{FarmingPool, FarmingPosition};

    #[test]
    fn apy_reward_calculation_accuracy() {
        let one_year_rewards = calculate_rewards(1_000_000, 1_000, SECONDS_PER_YEAR as u64);
        assert_eq!(one_year_rewards, 100_000);
    }

    #[test]
    fn reward_accrual_and_harvest_ready_amount() {
        let pool = FarmingPool {
            id: 1,
            lp_token: Address::from_string(&"".into()),
            reward_token: Address::from_string(&"".into()),
            reward_rate_bps: 2_000,
            lock_period: 100,
            total_staked: 0,
            created_at: 0,
        };

        let staker = Address::from_string(&"".into());
        let mut position = FarmingPosition {
            staker,
            amount: 500_000,
            last_update: 0,
            stake_timestamp: 0,
            pending_rewards: 0,
        };

        accrue_rewards(&pool, &mut position, SECONDS_PER_YEAR as u64 / 2);
        assert_eq!(position.pending_rewards, 50_000);
    }

    #[test]
    fn lock_period_check() {
        assert!(!lock_expired(1_000, 500, 1_499));
        assert!(lock_expired(1_000, 500, 1_500));
    }
}
