//! Rebalancing logic for automated strategy adjustments

use soroban_sdk::{contracterror, panic_with_error, Env, Vec};

use super::{AllocationTarget, DataKey, StrategyConfig};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum RebalancingError {
    /// Rebalancing not needed (drift within threshold)
    RebalancingNotNeeded = 1,
    /// Invalid target allocation
    InvalidTargetAllocation = 2,
    /// Rebalancing too frequent
    TooFrequentRebalancing = 3,
    /// Strategy not found
    StrategyNotFound = 4,
    /// Rebalancing calculation failed
    CalculationFailed = 5,
}

/// Check if a strategy needs rebalancing
pub fn should_rebalance(env: &Env, strategy_id: u64) -> Result<bool, RebalancingError> {
    let strategy = super::get_strategy(env, strategy_id)
        .ok_or(RebalancingError::StrategyNotFound)?;

    let now = env.ledger().timestamp();

    // Check frequency constraint
    if now < strategy.last_rebalance + strategy.rebalance_frequency_seconds {
        return Err(RebalancingError::TooFrequentRebalancing);
    }

    Ok(true)
}

/// Calculate rebalancing adjustments required
pub fn calculate_rebalancing(
    env: &Env,
    strategy_id: u64,
    current_allocations: &Vec<i128>,
    total_aum: i128,
) -> Result<Vec<i128>, RebalancingError> {
    let strategy = super::get_strategy(env, strategy_id)
        .ok_or(RebalancingError::StrategyNotFound)?;

    if total_aum <= 0 {
        panic_with_error!(env, RebalancingError::CalculationFailed);
    }

    let mut adjustments = Vec::new(env);

    for (i, target) in strategy.allocations.iter().enumerate() {
        if i >= current_allocations.len() {
            break;
        }

        let current_amount = current_allocations
            .get(i)
            .ok_or(RebalancingError::CalculationFailed)?;

        // Calculate target amount
        let target_amount = (total_aum as u128)
            .checked_mul(target.allocation_bps as u128)
            .ok_or(RebalancingError::CalculationFailed)?
            .checked_div(10000)
            .ok_or(RebalancingError::CalculationFailed)? as i128;

        // Calculate deviation from target
        let deviation = target_amount - current_amount;

        // Calculate drift in basis points
        let drift_bps = if current_amount != 0 {
            ((deviation.abs() as u128)
                .checked_mul(10000)
                .ok_or(RebalancingError::CalculationFailed)?
                .checked_div(current_amount.abs() as u128)
                .ok_or(RebalancingError::CalculationFailed)?) as u32
        } else {
            10001 // Force rebalance if no current allocation
        };

        // Check if deviation exceeds threshold
        if drift_bps <= strategy.rebalance_threshold_bps {
            adjustments.push_back(0);
            continue;
        }

        adjustments.push_back(deviation);
    }

    Ok(adjustments)
}

/// Execute rebalancing for a strategy
pub fn execute_rebalancing(
    env: &Env,
    strategy_id: u64,
) -> Result<(), RebalancingError> {
    let mut strategy = super::get_strategy(env, strategy_id)
        .ok_or(RebalancingError::StrategyNotFound)?;

    let now = env.ledger().timestamp();

    // Update last rebalance timestamp
    strategy.last_rebalance = now;

    super::update_strategy(env, &strategy);

    Ok(())
}

/// Validate allocation targets for rebalancing
pub fn validate_allocations(
    env: &Env,
    allocations: &Vec<AllocationTarget>,
) -> Result<(), RebalancingError> {
    let mut total_bps = 0u32;

    for allocation in allocations {
        if allocation.allocation_bps > 10000 {
            panic_with_error!(env, RebalancingError::InvalidTargetAllocation);
        }

        if allocation.min_allocation_bps > allocation.allocation_bps {
            panic_with_error!(env, RebalancingError::InvalidTargetAllocation);
        }

        if allocation.max_allocation_bps < allocation.allocation_bps {
            panic_with_error!(env, RebalancingError::InvalidTargetAllocation);
        }

        total_bps = total_bps
            .checked_add(allocation.allocation_bps)
            .ok_or(RebalancingError::InvalidTargetAllocation)?;

        if total_bps > 10000 {
            panic_with_error!(env, RebalancingError::InvalidTargetAllocation);
        }
    }

    Ok(())
}

/// Get rebalancing metrics for a strategy
pub fn get_rebalancing_metrics(
    env: &Env,
    strategy_id: u64,
    current_allocations: &Vec<i128>,
    total_aum: i128,
) -> Result<RebalancingMetrics, RebalancingError> {
    let strategy = super::get_strategy(env, strategy_id)
        .ok_or(RebalancingError::StrategyNotFound)?;

    let now = env.ledger().timestamp();
    let time_since_last_rebalance = now - strategy.last_rebalance;
    let time_until_next_rebalance = if time_since_last_rebalance >= strategy.rebalance_frequency_seconds {
        0
    } else {
        strategy.rebalance_frequency_seconds - time_since_last_rebalance
    };

    let should_rebalance = time_until_next_rebalance == 0;
    let max_drift = calculate_max_drift(env, strategy_id, current_allocations, total_aum)?;

    Ok(RebalancingMetrics {
        last_rebalance: strategy.last_rebalance,
        next_rebalance: strategy.last_rebalance + strategy.rebalance_frequency_seconds,
        time_until_next_rebalance,
        should_rebalance,
        max_drift_bps: max_drift,
        threshold_bps: strategy.rebalance_threshold_bps,
    })
}

/// Calculate maximum drift across all allocations
fn calculate_max_drift(
    env: &Env,
    strategy_id: u64,
    current_allocations: &Vec<i128>,
    total_aum: i128,
) -> Result<u32, RebalancingError> {
    let strategy = super::get_strategy(env, strategy_id)
        .ok_or(RebalancingError::StrategyNotFound)?;

    let mut max_drift = 0u32;

    for (i, target) in strategy.allocations.iter().enumerate() {
        if i >= current_allocations.len() {
            break;
        }

        let current_amount = current_allocations
            .get(i)
            .ok_or(RebalancingError::CalculationFailed)?;

        let target_amount = (total_aum as u128)
            .checked_mul(target.allocation_bps as u128)
            .ok_or(RebalancingError::CalculationFailed)?
            .checked_div(10000)
            .ok_or(RebalancingError::CalculationFailed)? as i128;

        let deviation = (target_amount - current_amount).abs();

        let drift_bps = if current_amount != 0 {
            ((deviation as u128)
                .checked_mul(10000)
                .ok_or(RebalancingError::CalculationFailed)?
                .checked_div(current_amount.abs() as u128)
                .ok_or(RebalancingError::CalculationFailed)?) as u32
        } else {
            10001
        };

        if drift_bps > max_drift {
            max_drift = drift_bps;
        }
    }

    Ok(max_drift)
}

/// Metrics for strategy rebalancing
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RebalancingMetrics {
    /// Last timestamp when rebalancing was performed
    pub last_rebalance: u64,
    /// Next scheduled rebalance timestamp
    pub next_rebalance: u64,
    /// Seconds until next rebalance
    pub time_until_next_rebalance: u64,
    /// Whether rebalancing should occur now
    pub should_rebalance: bool,
    /// Maximum drift from target allocation in basis points
    pub max_drift_bps: u32,
    /// Rebalance threshold in basis points
    pub threshold_bps: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rebalancing_errors() {
        // Verify error codes are unique
        assert_ne!(
            RebalancingError::RebalancingNotNeeded as u32,
            RebalancingError::InvalidTargetAllocation as u32
        );
        assert_ne!(
            RebalancingError::TooFrequentRebalancing as u32,
            RebalancingError::StrategyNotFound as u32
        );
    }
}
