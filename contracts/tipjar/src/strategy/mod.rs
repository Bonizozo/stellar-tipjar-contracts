//! Automated Strategy Execution Module
//!
//! This module provides automated strategy execution for tip investments and yield optimization.
//! It supports multiple strategy types, automatic rebalancing, performance tracking, and history management.

pub mod execution;
pub mod history;
pub mod performance;
pub mod rebalancing;

use soroban_sdk::{contracttype, Address};

/// Strategy types for automated tip investment and yield optimization
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrategyType {
    /// Conservative strategy: low-risk, stable yield
    Conservative = 0,
    /// Balanced strategy: moderate risk and yield
    Balanced = 1,
    /// Aggressive strategy: high-risk, high yield potential
    Aggressive = 2,
    /// Index-tracking strategy: follows market index
    IndexTracking = 3,
    /// Liquidity providing strategy: provides LP to AMMs
    LiquidityProviding = 4,
    /// Yield farming strategy: farm tokens for rewards
    YieldFarming = 5,
    /// Staking strategy: stake tokens for rewards
    Staking = 6,
    /// Custom strategy: user-defined allocations
    Custom = 7,
}

/// Allocation target for a specific pool or investment
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocationTarget {
    /// Pool or investment identifier
    pub target_id: Address,
    /// Percentage allocation in basis points (0-10000)
    pub allocation_bps: u32,
    /// Minimum allocation to maintain
    pub min_allocation_bps: u32,
    /// Maximum allocation allowed
    pub max_allocation_bps: u32,
}

/// Strategy configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrategyConfig {
    /// Strategy identifier
    pub id: u64,
    /// Strategy type
    pub strategy_type: StrategyType,
    /// Strategy owner
    pub owner: Address,
    /// Allocations for this strategy
    /// Stored as a vector of AllocationTarget
    pub allocations: soroban_sdk::Vec<AllocationTarget>,
    /// Minimum rebalance threshold in basis points
    /// Only rebalance if allocation drifts more than this
    pub rebalance_threshold_bps: u32,
    /// Rebalance frequency in seconds
    pub rebalance_frequency_seconds: u64,
    /// Last rebalance timestamp
    pub last_rebalance: u64,
    /// Performance fee in basis points (taken from returns)
    pub performance_fee_bps: u32,
    /// Management fee in basis points (annual)
    pub management_fee_bps: u32,
    /// Total assets under management (in smallest units)
    pub total_aum: i128,
    /// Creation timestamp
    pub created_at: u64,
    /// Is strategy active
    pub active: bool,
}

/// Individual allocation instance for a specific user/entity
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocationInstance {
    /// Strategy identifier
    pub strategy_id: u64,
    /// User/owner address
    pub owner: Address,
    /// Amount allocated to this target (smallest units)
    pub amount: i128,
    /// Timestamp of last allocation
    pub allocated_at: u64,
    /// Pending rewards not yet harvested
    pub pending_rewards: i128,
}

/// Storage keys for strategy module
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    StrategyConfig(u64),
    AllocationInstance(u64, Address),
    StrategyNextId,
    UserStrategies(Address),
    StrategyPerformance(u64),
    StrategyHistory(u64, u64),
}

/// Initialize strategy module with default state
pub fn init_strategy_module(env: &soroban_sdk::Env) {
    env.storage().persistent().set(&DataKey::StrategyNextId, &1u64);
}

/// Get the next strategy ID
pub fn get_next_strategy_id(env: &soroban_sdk::Env) -> u64 {
    let next_id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::StrategyNextId)
        .unwrap_or(1);
    env.storage()
        .persistent()
        .set(&DataKey::StrategyNextId, &(next_id + 1));
    next_id
}

/// Create a new strategy
pub fn create_strategy(
    env: &soroban_sdk::Env,
    strategy_type: StrategyType,
    owner: Address,
    allocations: soroban_sdk::Vec<AllocationTarget>,
    rebalance_threshold_bps: u32,
    rebalance_frequency_seconds: u64,
    performance_fee_bps: u32,
    management_fee_bps: u32,
) -> u64 {
    let strategy_id = get_next_strategy_id(env);
    let now = env.ledger().timestamp();

    let strategy = StrategyConfig {
        id: strategy_id,
        strategy_type,
        owner,
        allocations,
        rebalance_threshold_bps,
        rebalance_frequency_seconds,
        last_rebalance: now,
        performance_fee_bps,
        management_fee_bps,
        total_aum: 0,
        created_at: now,
        active: true,
    };

    env.storage()
        .persistent()
        .set(&DataKey::StrategyConfig(strategy_id), &strategy);

    strategy_id
}

/// Get strategy configuration
pub fn get_strategy(env: &soroban_sdk::Env, strategy_id: u64) -> Option<StrategyConfig> {
    env.storage()
        .persistent()
        .get(&DataKey::StrategyConfig(strategy_id))
}

/// Get strategy or panic if not found
pub fn get_strategy_or_panic(env: &soroban_sdk::Env, strategy_id: u64) -> StrategyConfig {
    get_strategy(env, strategy_id).expect("Strategy not found")
}

/// Update strategy configuration
pub fn update_strategy(env: &soroban_sdk::Env, strategy: &StrategyConfig) {
    env.storage()
        .persistent()
        .set(&DataKey::StrategyConfig(strategy.id), strategy);
}

/// Get allocation instance
pub fn get_allocation_instance(
    env: &soroban_sdk::Env,
    strategy_id: u64,
    owner: &Address,
) -> Option<AllocationInstance> {
    env.storage()
        .persistent()
        .get(&DataKey::AllocationInstance(strategy_id, owner.clone()))
}

/// Update allocation instance
pub fn update_allocation_instance(
    env: &soroban_sdk::Env,
    strategy_id: u64,
    owner: &Address,
    allocation: &AllocationInstance,
) {
    env.storage()
        .persistent()
        .set(&DataKey::AllocationInstance(strategy_id, owner.clone()), allocation);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_type_enum() {
        // Verify enum values are unique
        assert_ne!(StrategyType::Conservative as u32, StrategyType::Balanced as u32);
        assert_ne!(StrategyType::Balanced as u32, StrategyType::Aggressive as u32);
        assert_ne!(StrategyType::Aggressive as u32, StrategyType::IndexTracking as u32);
    }
}
