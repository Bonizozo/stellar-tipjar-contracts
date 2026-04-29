//! Strategy execution logic for automated investment and yield optimization

use soroban_sdk::{contracterror, panic_with_error, Address, Env, Vec};

use super::{AllocationInstance, AllocationTarget, DataKey, StrategyConfig, StrategyType};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum ExecutionError {
    /// Strategy not found
    StrategyNotFound = 1,
    /// Invalid allocation
    InvalidAllocation = 2,
    /// Allocation exceeds 100%
    AllocationExceedsCap = 3,
    /// Insufficient balance
    InsufficientBalance = 4,
    /// Execution failed
    ExecutionFailed = 5,
    /// Strategy not active
    StrategyNotActive = 6,
}

/// Execute a strategy for a user
pub fn execute_strategy(
    env: &Env,
    strategy_id: u64,
    owner: Address,
    total_amount: i128,
) -> Result<Vec<i128>, ExecutionError> {
    // Get strategy
    let strategy = super::get_strategy(env, strategy_id)
        .ok_or(ExecutionError::StrategyNotFound)?;

    if !strategy.active {
        panic_with_error!(env, ExecutionError::StrategyNotActive);
    }

    // Validate allocations and get distribution
    let distribution = calculate_distribution(env, &strategy, total_amount)?;

    // Execute allocations based on strategy type
    execute_allocations(env, &strategy, owner, &distribution)?;

    Ok(distribution)
}

/// Calculate distribution amounts based on strategy allocations
pub fn calculate_distribution(
    env: &Env,
    strategy: &StrategyConfig,
    total_amount: i128,
) -> Result<Vec<i128>, ExecutionError> {
    let mut distribution = Vec::new(env);

    // Validate allocations and calculate amounts
    let mut total_allocation_bps = 0u32;

    for allocation in &strategy.allocations {
        // Check allocation bounds
        if allocation.allocation_bps > 10000 {
            panic_with_error!(env, ExecutionError::InvalidAllocation);
        }

        total_allocation_bps = total_allocation_bps
            .checked_add(allocation.allocation_bps)
            .ok_or(ExecutionError::AllocationExceedsCap)?;

        let amount = (total_amount as u128)
            .checked_mul(allocation.allocation_bps as u128)
            .ok_or(ExecutionError::InvalidAllocation)?
            .checked_div(10000)
            .ok_or(ExecutionError::InvalidAllocation)? as i128;

        distribution.push_back(amount);
    }

    // Verify allocations sum to 100% or less
    if total_allocation_bps > 10000 {
        panic_with_error!(env, ExecutionError::AllocationExceedsCap);
    }

    Ok(distribution)
}

/// Execute allocations to respective targets
pub fn execute_allocations(
    env: &Env,
    strategy: &StrategyConfig,
    owner: Address,
    distribution: &Vec<i128>,
) -> Result<(), ExecutionError> {
    let now = env.ledger().timestamp();

    for (i, target) in strategy.allocations.iter().enumerate() {
        if i >= distribution.len() {
            break;
        }

        let amount = distribution.get(i).ok_or(ExecutionError::ExecutionFailed)?;

        if amount <= 0 {
            continue;
        }

        // Execute based on strategy type
        match strategy.strategy_type {
            StrategyType::YieldFarming => {
                execute_yield_farming(env, &owner, &target, amount)?;
            }
            StrategyType::Staking => {
                execute_staking(env, &owner, &target, amount)?;
            }
            StrategyType::LiquidityProviding => {
                execute_liquidity_providing(env, &owner, &target, amount)?;
            }
            StrategyType::Conservative
            | StrategyType::Balanced
            | StrategyType::Aggressive
            | StrategyType::IndexTracking
            | StrategyType::Custom => {
                // Default execution: allocate to target
                allocate_to_target(env, &owner, strategy.id, &target, amount)?;
            }
        }
    }

    Ok(())
}

/// Execute yield farming allocation
fn execute_yield_farming(
    env: &Env,
    _owner: &Address,
    _target: &AllocationTarget,
    _amount: i128,
) -> Result<(), ExecutionError> {
    // In a real implementation, this would:
    // 1. Approve the farming pool to spend tokens
    // 2. Stake the tokens in the pool
    // 3. Track the position
    // For now, this is a placeholder
    Ok(())
}

/// Execute staking allocation
fn execute_staking(
    env: &Env,
    _owner: &Address,
    _target: &AllocationTarget,
    _amount: i128,
) -> Result<(), ExecutionError> {
    // In a real implementation, this would:
    // 1. Approve the staking contract to spend tokens
    // 2. Stake the tokens
    // 3. Track the position
    // For now, this is a placeholder
    Ok(())
}

/// Execute liquidity providing allocation
fn execute_liquidity_providing(
    env: &Env,
    _owner: &Address,
    _target: &AllocationTarget,
    _amount: i128,
) -> Result<(), ExecutionError> {
    // In a real implementation, this would:
    // 1. Approve the AMM to spend tokens
    // 2. Provide liquidity
    // 3. Receive LP tokens
    // 4. Track the position
    // For now, this is a placeholder
    Ok(())
}

/// Allocate amount to a specific target
fn allocate_to_target(
    env: &Env,
    owner: &Address,
    strategy_id: u64,
    _target: &AllocationTarget,
    amount: i128,
) -> Result<(), ExecutionError> {
    let now = env.ledger().timestamp();

    // Get or create allocation instance
    let mut allocation = super::get_allocation_instance(env, strategy_id, owner)
        .unwrap_or(AllocationInstance {
            strategy_id,
            owner: owner.clone(),
            amount: 0,
            allocated_at: now,
            pending_rewards: 0,
        });

    // Update allocation
    allocation.amount = allocation
        .amount
        .checked_add(amount)
        .ok_or(ExecutionError::InvalidAllocation)?;
    allocation.allocated_at = now;

    super::update_allocation_instance(env, strategy_id, owner, &allocation);

    Ok(())
}

/// Get distribution for a specific strategy and amount
pub fn get_strategy_distribution(
    env: &Env,
    strategy_id: u64,
    total_amount: i128,
) -> Result<Vec<i128>, ExecutionError> {
    let strategy = super::get_strategy(env, strategy_id)
        .ok_or(ExecutionError::StrategyNotFound)?;

    calculate_distribution(env, &strategy, total_amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_errors() {
        // Verify error codes are unique
        assert_ne!(
            ExecutionError::StrategyNotFound as u32,
            ExecutionError::InvalidAllocation as u32
        );
        assert_ne!(
            ExecutionError::InvalidAllocation as u32,
            ExecutionError::AllocationExceedsCap as u32
        );
    }
}
