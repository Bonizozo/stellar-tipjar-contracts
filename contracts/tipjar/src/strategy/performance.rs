//! Performance tracking and calculation for strategies

use soroban_sdk::{contracterror, panic_with_error, Env};

use super::DataKey;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum PerformanceError {
    /// Strategy not found
    StrategyNotFound = 1,
    /// Performance data not found
    PerformanceDataNotFound = 2,
    /// Invalid calculation
    InvalidCalculation = 3,
    /// No performance history available
    NoPerformanceHistory = 4,
}

/// Strategy performance metrics
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformanceMetrics {
    /// Strategy identifier
    pub strategy_id: u64,
    /// Total return in smallest units
    pub total_return: i128,
    /// Return percentage in basis points (1 bps = 0.01%)
    pub return_percentage_bps: i32,
    /// Unrealized gains
    pub unrealized_gains: i128,
    /// Realized gains
    pub realized_gains: i128,
    /// Total fees paid
    pub total_fees: i128,
    /// Performance fees paid
    pub performance_fees: i128,
    /// Management fees paid
    pub management_fees: i128,
    /// Net return after fees
    pub net_return: i128,
    /// Number of days tracked
    pub days_tracked: u64,
    /// Annualized return in basis points
    pub annualized_return_bps: i32,
    /// Strategy creation timestamp
    pub created_at: u64,
    /// Last update timestamp
    pub updated_at: u64,
}

/// Calculate performance metrics for a strategy
pub fn calculate_performance(
    env: &Env,
    strategy_id: u64,
    initial_investment: i128,
    current_value: i128,
    accumulated_rewards: i128,
    performance_fee_bps: u32,
    management_fee_bps: u32,
    created_at: u64,
) -> Result<PerformanceMetrics, PerformanceError> {
    let now = env.ledger().timestamp();

    if current_value < 0 || initial_investment <= 0 {
        panic_with_error!(env, PerformanceError::InvalidCalculation);
    }

    // Calculate gross return
    let gross_return = current_value
        .checked_add(accumulated_rewards)
        .ok_or(PerformanceError::InvalidCalculation)?
        .checked_sub(initial_investment)
        .ok_or(PerformanceError::InvalidCalculation)?;

    // Calculate time period in days
    let time_elapsed = now - created_at;
    let days_tracked = time_elapsed / 86400; // 86400 seconds in a day

    // Calculate return percentage in basis points
    let return_percentage_bps = if initial_investment != 0 {
        ((gross_return as i64)
            .checked_mul(10000)
            .ok_or(PerformanceError::InvalidCalculation)?
            .checked_div(initial_investment as i64)
            .ok_or(PerformanceError::InvalidCalculation)?) as i32
    } else {
        0
    };

    // Calculate fees
    let performance_over_zero = if gross_return > 0 {
        gross_return
    } else {
        0
    };

    let performance_fees = (performance_over_zero as u128)
        .checked_mul(performance_fee_bps as u128)
        .ok_or(PerformanceError::InvalidCalculation)?
        .checked_div(10000)
        .ok_or(PerformanceError::InvalidCalculation)? as i128;

    let management_fee_amount = if days_tracked > 0 {
        (initial_investment as u128)
            .checked_mul(management_fee_bps as u128)
            .ok_or(PerformanceError::InvalidCalculation)?
            .checked_mul(days_tracked as u128)
            .ok_or(PerformanceError::InvalidCalculation)?
            .checked_div(365 * 10000)
            .ok_or(PerformanceError::InvalidCalculation)? as i128
    } else {
        0
    };

    let total_fees = performance_fees
        .checked_add(management_fee_amount)
        .ok_or(PerformanceError::InvalidCalculation)?;

    let net_return = gross_return
        .checked_sub(total_fees)
        .ok_or(PerformanceError::InvalidCalculation)?;

    // Calculate annualized return
    let annualized_return_bps = if days_tracked > 0 {
        let daily_return = return_percentage_bps as i64;
        let annualized = daily_return
            .checked_mul(365)
            .ok_or(PerformanceError::InvalidCalculation)?
            .checked_div(days_tracked as i64)
            .ok_or(PerformanceError::InvalidCalculation)?;
        annualized as i32
    } else {
        0
    };

    Ok(PerformanceMetrics {
        strategy_id,
        total_return: gross_return,
        return_percentage_bps,
        unrealized_gains: current_value - initial_investment,
        realized_gains: accumulated_rewards,
        total_fees,
        performance_fees,
        management_fees: management_fee_amount,
        net_return,
        days_tracked,
        annualized_return_bps,
        created_at,
        updated_at: now,
    })
}

/// Store performance metrics
pub fn store_performance_metrics(
    env: &Env,
    metrics: &PerformanceMetrics,
) {
    env.storage()
        .persistent()
        .set(&DataKey::StrategyPerformance(metrics.strategy_id), metrics);
}

/// Get performance metrics
pub fn get_performance_metrics(
    env: &Env,
    strategy_id: u64,
) -> Result<PerformanceMetrics, PerformanceError> {
    env.storage()
        .persistent()
        .get(&DataKey::StrategyPerformance(strategy_id))
        .ok_or(PerformanceError::PerformanceDataNotFound)
}

/// Calculate Sharpe ratio (simplified - no volatility calculation)
pub fn calculate_risk_adjusted_return(
    env: &Env,
    total_return: i128,
    risk_free_rate_bps: u32,
    num_days: u64,
) -> Result<i32, PerformanceError> {
    if num_days == 0 {
        return Ok(0);
    }

    let daily_risk_free_rate = (risk_free_rate_bps as i64)
        .checked_div(365)
        .ok_or(PerformanceError::InvalidCalculation)? as i32;

    let daily_return = if num_days > 0 {
        (total_return as i32)
            .checked_div(num_days as i32)
            .ok_or(PerformanceError::InvalidCalculation)?
    } else {
        0
    };

    Ok(daily_return - daily_risk_free_rate)
}

/// Compare strategies by performance
pub fn compare_strategies(
    env: &Env,
    strategy_ids: &soroban_sdk::Vec<u64>,
) -> Result<Vec<StrategyComparison>, PerformanceError> {
    let mut comparisons = soroban_sdk::Vec::new(env);

    for strategy_id in strategy_ids {
        if let Ok(metrics) = get_performance_metrics(env, strategy_id) {
            comparisons.push_back(StrategyComparison {
                strategy_id: strategy_id,
                return_percentage_bps: metrics.return_percentage_bps,
                annualized_return_bps: metrics.annualized_return_bps,
                total_fees: metrics.total_fees,
                net_return: metrics.net_return,
            });
        }
    }

    Ok(comparisons)
}

/// Strategy comparison data
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrategyComparison {
    pub strategy_id: u64,
    pub return_percentage_bps: i32,
    pub annualized_return_bps: i32,
    pub total_fees: i128,
    pub net_return: i128,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_errors() {
        // Verify error codes are unique
        assert_ne!(
            PerformanceError::StrategyNotFound as u32,
            PerformanceError::PerformanceDataNotFound as u32
        );
        assert_ne!(
            PerformanceError::InvalidCalculation as u32,
            PerformanceError::NoPerformanceHistory as u32
        );
    }
}
