//! Strategy execution history tracking

use soroban_sdk::{contracterror, panic_with_error, Env, Vec};

use super::DataKey;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum HistoryError {
    /// Strategy not found
    StrategyNotFound = 1,
    /// History entry not found
    HistoryNotFound = 2,
    /// Invalid history data
    InvalidHistoryData = 3,
}

/// History entry for a strategy execution or rebalancing
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryEntry {
    /// Unique entry ID
    pub entry_id: u64,
    /// Strategy identifier
    pub strategy_id: u64,
    /// Type of event
    pub event_type: HistoryEventType,
    /// Timestamp of event
    pub timestamp: u64,
    /// Amount involved in transaction
    pub amount: i128,
    /// Details about the event
    pub details: soroban_sdk::String,
    /// Performance snapshot at time of event
    pub performance_snapshot: Option<PerformanceSnapshot>,
}

/// Types of history events
#[soroban_sdk::contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryEventType {
    /// Strategy allocation
    Allocation = 0,
    /// Strategy rebalancing
    Rebalancing = 1,
    /// Harvest/claim rewards
    Harvest = 2,
    /// Strategy modification
    Modification = 3,
    /// Fee collection
    FeeCollection = 4,
}

/// Performance snapshot at time of event
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformanceSnapshot {
    /// Total AUM at snapshot time
    pub total_aum: i128,
    /// Current return in basis points
    pub return_bps: i32,
    /// Realized gains
    pub realized_gains: i128,
    /// Fees paid in this period
    pub period_fees: i128,
}

/// Track a strategy execution event
pub fn record_history_entry(
    env: &Env,
    strategy_id: u64,
    event_type: HistoryEventType,
    amount: i128,
    details: soroban_sdk::String,
) -> Result<u64, HistoryError> {
    let now = env.ledger().timestamp();
    let entry_id = generate_entry_id(env, strategy_id);

    let entry = HistoryEntry {
        entry_id,
        strategy_id,
        event_type,
        timestamp: now,
        amount,
        details,
        performance_snapshot: None,
    };

    env.storage()
        .persistent()
        .set(&DataKey::StrategyHistory(strategy_id, entry_id), &entry);

    Ok(entry_id)
}

/// Record history entry with performance snapshot
pub fn record_history_with_performance(
    env: &Env,
    strategy_id: u64,
    event_type: HistoryEventType,
    amount: i128,
    details: soroban_sdk::String,
    performance: PerformanceSnapshot,
) -> Result<u64, HistoryError> {
    let now = env.ledger().timestamp();
    let entry_id = generate_entry_id(env, strategy_id);

    let entry = HistoryEntry {
        entry_id,
        strategy_id,
        event_type,
        timestamp: now,
        amount,
        details,
        performance_snapshot: Some(performance),
    };

    env.storage()
        .persistent()
        .set(&DataKey::StrategyHistory(strategy_id, entry_id), &entry);

    Ok(entry_id)
}

/// Get a history entry
pub fn get_history_entry(
    env: &Env,
    strategy_id: u64,
    entry_id: u64,
) -> Result<HistoryEntry, HistoryError> {
    env.storage()
        .persistent()
        .get(&DataKey::StrategyHistory(strategy_id, entry_id))
        .ok_or(HistoryError::HistoryNotFound)
}

/// Generate unique entry ID for strategy history
fn generate_entry_id(env: &Env, strategy_id: u64) -> u64 {
    // Use timestamp + strategy_id as unique ID
    let now = env.ledger().timestamp();
    ((now << 16) | (strategy_id & 0xFFFF))
}

/// Get strategy execution statistics
pub fn get_execution_statistics(
    env: &Env,
    strategy_id: u64,
    event_types: &Vec<HistoryEventType>,
) -> Result<ExecutionStatistics, HistoryError> {
    let strategy = super::get_strategy(env, strategy_id)
        .ok_or(HistoryError::StrategyNotFound)?;

    let created_at = strategy.created_at;
    let now = env.ledger().timestamp();

    let mut total_amount_allocated = 0i128;
    let mut total_fees_collected = 0i128;
    let mut num_allocations = 0u64;
    let mut num_rebalances = 0u64;
    let mut num_harvests = 0u64;

    // In a real implementation, we would iterate through history entries
    // For now, we provide a structure that can be used
    Ok(ExecutionStatistics {
        strategy_id,
        total_amount_allocated,
        total_fees_collected,
        num_allocations,
        num_rebalances,
        num_harvests,
        days_active: (now - created_at) / 86400,
        created_at,
        last_activity: now,
    })
}

/// Strategy execution statistics
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionStatistics {
    /// Strategy identifier
    pub strategy_id: u64,
    /// Total amount allocated across all events
    pub total_amount_allocated: i128,
    /// Total fees collected
    pub total_fees_collected: i128,
    /// Number of allocation events
    pub num_allocations: u64,
    /// Number of rebalancing events
    pub num_rebalances: u64,
    /// Number of harvest events
    pub num_harvests: u64,
    /// Days strategy has been active
    pub days_active: u64,
    /// Strategy creation time
    pub created_at: u64,
    /// Last recorded activity
    pub last_activity: u64,
}

/// Get total fees collected for a strategy
pub fn get_total_fees(
    env: &Env,
    strategy_id: u64,
) -> Result<i128, HistoryError> {
    let _strategy = super::get_strategy(env, strategy_id)
        .ok_or(HistoryError::StrategyNotFound)?;

    // In a real implementation, this would sum all fee collection entries
    Ok(0)
}

/// Get audit trail for a strategy
pub fn get_audit_trail(
    env: &Env,
    strategy_id: u64,
    max_entries: u32,
) -> Result<Vec<HistoryEntry>, HistoryError> {
    let _strategy = super::get_strategy(env, strategy_id)
        .ok_or(HistoryError::StrategyNotFound)?;

    // In a real implementation, this would retrieve recent entries
    // For now, return empty vector
    Ok(Vec::new(env))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_event_types() {
        // Verify enum values are unique
        assert_ne!(HistoryEventType::Allocation as u32, HistoryEventType::Rebalancing as u32);
        assert_ne!(HistoryEventType::Harvest as u32, HistoryEventType::Modification as u32);
    }

    #[test]
    fn test_history_errors() {
        // Verify error codes are unique
        assert_ne!(
            HistoryError::StrategyNotFound as u32,
            HistoryError::HistoryNotFound as u32
        );
        assert_ne!(
            HistoryError::HistoryNotFound as u32,
            HistoryError::InvalidHistoryData as u32
        );
    }
}
