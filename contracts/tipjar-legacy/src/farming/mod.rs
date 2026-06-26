//! Yield farming module.
//!
//! Users stake LP tokens in pools and accrue rewards over time.

pub mod pool;
pub mod rewards;

use soroban_sdk::{contracttype, Address};

/// A farming pool configuration and aggregate state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FarmingPool {
    /// Pool identifier.
    pub id: u64,
    /// LP token accepted for staking.
    pub lp_token: Address,
    /// Reward token distributed to stakers.
    pub reward_token: Address,
    /// APY in basis points.
    pub reward_rate_bps: u32,
    /// Lock duration in seconds before unstake is allowed.
    pub lock_period: u64,
    /// Total LP amount currently staked in this pool.
    pub total_staked: i128,
    /// Creation timestamp.
    pub created_at: u64,
}

/// Staker position in a farming pool.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FarmingPosition {
    /// Staker account.
    pub staker: Address,
    /// Current staked amount.
    pub amount: i128,
    /// Last timestamp rewards were accrued.
    pub last_update: u64,
    /// Timestamp of initial/last stake used for lock checks.
    pub stake_timestamp: u64,
    /// Rewards accrued but not yet harvested.
    pub pending_rewards: i128,
}
