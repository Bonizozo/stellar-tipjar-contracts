#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Env as _},
    vec as soroban_vec, Address, Env, String,
};

use tipjar::strategy::{
    execution::{self, ExecutionError},
    history::{self, HistoryEventType},
    performance::{self, PerformanceMetrics},
    rebalancing::{self, RebalancingMetrics},
    AllocationInstance, AllocationTarget, StrategyConfig, StrategyType,
};

#[test]
fn test_create_strategy() {
    let env = Env::default();
    env.mock_all_auctions();

    tipjar::strategy::init_strategy_module(&env);

    let owner = Address::random(&env);
    let target = Address::random(&env);

    let mut allocations = soroban_vec![&env];
    allocations.push_back(AllocationTarget {
        target_id: target.clone(),
        allocation_bps: 10000, // 100%
        min_allocation_bps: 5000,
        max_allocation_bps: 10000,
    });

    let strategy_id = tipjar::strategy::create_strategy(
        &env,
        StrategyType::Conservative,
        owner.clone(),
        allocations.clone(),
        100,  // rebalance threshold: 1%
        86400, // rebalance frequency: 1 day
        200,  // performance fee: 2%
        100,  // management fee: 1%
    );

    assert_eq!(strategy_id, 1);

    // Verify strategy was created
    let strategy = tipjar::strategy::get_strategy(&env, strategy_id).unwrap();
    assert_eq!(strategy.id, strategy_id);
    assert_eq!(strategy.owner, owner);
    assert_eq!(strategy.total_aum, 0);
    assert!(strategy.active);
}

#[test]
fn test_strategy_execution() {
    let env = Env::default();
    env.mock_all_auctions();

    tipjar::strategy::init_strategy_module(&env);

    let owner = Address::random(&env);
    let target = Address::random(&env);

    let mut allocations = soroban_vec![&env];
    allocations.push_back(AllocationTarget {
        target_id: target.clone(),
        allocation_bps: 10000,
        min_allocation_bps: 5000,
        max_allocation_bps: 10000,
    });

    let strategy_id = tipjar::strategy::create_strategy(
        &env,
        StrategyType::Balanced,
        owner.clone(),
        allocations,
        100,
        86400,
        200,
        100,
    );

    // Execute strategy with 1000 units
    let result = execution::execute_strategy(&env, strategy_id, owner.clone(), 1000);

    assert!(result.is_ok());
    let distribution = result.unwrap();
    assert_eq!(distribution.len(), 1);
    assert_eq!(distribution.get(0).unwrap(), 1000);
}

#[test]
fn test_calculate_distribution() {
    let env = Env::default();
    env.mock_all_auctions();

    tipjar::strategy::init_strategy_module(&env);

    let owner = Address::random(&env);
    let target1 = Address::random(&env);
    let target2 = Address::random(&env);

    let mut allocations = soroban_vec![&env];
    allocations.push_back(AllocationTarget {
        target_id: target1.clone(),
        allocation_bps: 6000, // 60%
        min_allocation_bps: 5000,
        max_allocation_bps: 7000,
    });
    allocations.push_back(AllocationTarget {
        target_id: target2.clone(),
        allocation_bps: 4000, // 40%
        min_allocation_bps: 3000,
        max_allocation_bps: 5000,
    });

    let strategy_id = tipjar::strategy::create_strategy(
        &env,
        StrategyType::Balanced,
        owner,
        allocations,
        100,
        86400,
        200,
        100,
    );

    let strategy = tipjar::strategy::get_strategy(&env, strategy_id).unwrap();

    // Calculate distribution for 1000 units
    let result = execution::calculate_distribution(&env, &strategy, 1000);

    assert!(result.is_ok());
    let distribution = result.unwrap();
    assert_eq!(distribution.len(), 2);
    assert_eq!(distribution.get(0).unwrap(), 600); // 60% of 1000
    assert_eq!(distribution.get(1).unwrap(), 400); // 40% of 1000
}

#[test]
fn test_allocation_exceeds_cap() {
    let env = Env::default();
    env.mock_all_auctions();

    tipjar::strategy::init_strategy_module(&env);

    let owner = Address::random(&env);
    let target1 = Address::random(&env);
    let target2 = Address::random(&env);

    let mut allocations = soroban_vec![&env];
    allocations.push_back(AllocationTarget {
        target_id: target1,
        allocation_bps: 6000,
        min_allocation_bps: 5000,
        max_allocation_bps: 7000,
    });
    allocations.push_back(AllocationTarget {
        target_id: target2,
        allocation_bps: 5000, // Total 11000 > 10000
        min_allocation_bps: 4000,
        max_allocation_bps: 6000,
    });

    let strategy_id = tipjar::strategy::create_strategy(
        &env,
        StrategyType::Balanced,
        owner,
        allocations,
        100,
        86400,
        200,
        100,
    );

    let strategy = tipjar::strategy::get_strategy(&env, strategy_id).unwrap();

    // Should fail because allocations exceed 100%
    let result = execution::calculate_distribution(&env, &strategy, 1000);
    assert!(matches!(
        result,
        Err(ExecutionError::AllocationExceedsCap)
    ));
}

#[test]
fn test_rebalancing_metrics() {
    let env = Env::default();
    env.mock_all_auctions();

    tipjar::strategy::init_strategy_module(&env);

    let owner = Address::random(&env);
    let target = Address::random(&env);

    let mut allocations = soroban_vec![&env];
    allocations.push_back(AllocationTarget {
        target_id: target,
        allocation_bps: 10000,
        min_allocation_bps: 5000,
        max_allocation_bps: 10000,
    });

    let strategy_id = tipjar::strategy::create_strategy(
        &env,
        StrategyType::Balanced,
        owner,
        allocations,
        100, // rebalance threshold: 1%
        86400, // rebalance frequency: 1 day
        200,
        100,
    );

    let mut current_allocations = soroban_vec![&env];
    current_allocations.push_back(950); // 95% instead of 100%

    let result = rebalancing::get_rebalancing_metrics(
        &env,
        strategy_id,
        &current_allocations,
        1000,
    );

    assert!(result.is_ok());
    let metrics = result.unwrap();
    assert!(metrics.should_rebalance || metrics.time_until_next_rebalance == 0);
}

#[test]
fn test_performance_calculation() {
    let env = Env::default();
    env.mock_all_auctions();

    let strategy_id = 1u64;
    let initial_investment = 1000i128;
    let current_value = 1100i128;
    let accumulated_rewards = 50i128;
    let created_at = env.ledger().timestamp();

    let result = performance::calculate_performance(
        &env,
        strategy_id,
        initial_investment,
        current_value,
        accumulated_rewards,
        200, // performance fee: 2%
        100, // management fee: 1%
        created_at,
    );

    assert!(result.is_ok());
    let metrics = result.unwrap();
    assert_eq!(metrics.strategy_id, strategy_id);
    assert_eq!(metrics.total_return, 150); // 1100 + 50 - 1000
    assert!(metrics.return_percentage_bps > 1500); // ~15%
}

#[test]
fn test_strategy_types() {
    // Verify all strategy types have unique values
    let conservative = StrategyType::Conservative as u32;
    let balanced = StrategyType::Balanced as u32;
    let aggressive = StrategyType::Aggressive as u32;
    let index = StrategyType::IndexTracking as u32;
    let lp = StrategyType::LiquidityProviding as u32;
    let farming = StrategyType::YieldFarming as u32;
    let staking = StrategyType::Staking as u32;
    let custom = StrategyType::Custom as u32;

    let mut values = [conservative, balanced, aggressive, index, lp, farming, staking, custom];
    values.sort();

    for i in 0..values.len() - 1 {
        assert_ne!(values[i], values[i + 1], "Strategy types must have unique values");
    }
}

#[test]
fn test_allocation_validation() {
    let env = Env::default();
    env.mock_all_auctions();

    tipjar::strategy::init_strategy_module(&env);

    let target = Address::random(&env);

    let mut allocations = soroban_vec![&env];
    allocations.push_back(AllocationTarget {
        target_id: target,
        allocation_bps: 10000,
        min_allocation_bps: 5000,
        max_allocation_bps: 10000,
    });

    // Validate should succeed for valid allocations
    let result = rebalancing::validate_allocations(&env, &allocations);
    assert!(result.is_ok());
}

#[test]
fn test_multiple_allocations() {
    let env = Env::default();
    env.mock_all_auctions();

    tipjar::strategy::init_strategy_module(&env);

    let owner = Address::random(&env);
    let mut targets = Vec::new();
    let mut allocations = soroban_vec![&env];

    // Create 5 targets with 20% allocation each
    for i in 0..5 {
        let target = Address::random(&env);
        targets.push(target.clone());
        allocations.push_back(AllocationTarget {
            target_id: target,
            allocation_bps: 2000, // 20%
            min_allocation_bps: 1500,
            max_allocation_bps: 2500,
        });
    }

    let strategy_id = tipjar::strategy::create_strategy(
        &env,
        StrategyType::Balanced,
        owner.clone(),
        allocations,
        500, // 5% rebalance threshold
        86400,
        200,
        100,
    );

    // Execute with 10000 units
    let result = execution::execute_strategy(&env, strategy_id, owner, 10000);

    assert!(result.is_ok());
    let distribution = result.unwrap();
    assert_eq!(distribution.len(), 5);

    // Each should get 2000 (20%)
    for i in 0..5 {
        assert_eq!(distribution.get(i).unwrap(), 2000);
    }
}

#[test]
fn test_history_entry_record() {
    let env = Env::default();
    env.mock_all_auctions();

    tipjar::strategy::init_strategy_module(&env);

    let strategy_id = 1u64;
    let details = String::from_small_str(&env, "Test allocation");

    let result = history::record_history_entry(
        &env,
        strategy_id,
        HistoryEventType::Allocation,
        1000,
        details,
    );

    assert!(result.is_ok());
    let entry_id = result.unwrap();
    assert!(entry_id > 0);

    // Retrieve the entry
    let entry_result = history::get_history_entry(&env, strategy_id, entry_id);
    assert!(entry_result.is_ok());
}

#[test]
fn test_strategy_edge_cases() {
    let env = Env::default();
    env.mock_all_auctions();

    tipjar::strategy::init_strategy_module(&env);

    let owner = Address::random(&env);
    let target = Address::random(&env);

    let mut allocations = soroban_vec![&env];
    allocations.push_back(AllocationTarget {
        target_id: target,
        allocation_bps: 10000,
        min_allocation_bps: 0,
        max_allocation_bps: 10000,
    });

    let strategy_id = tipjar::strategy::create_strategy(
        &env,
        StrategyType::Conservative,
        owner.clone(),
        allocations,
        0,  // No minimum threshold
        1,  // Every second
        0,  // No performance fee
        0,  // No management fee
    );

    // Execute with 0 should work (edge case)
    let result = execution::execute_strategy(&env, strategy_id, owner, 0);
    assert!(result.is_ok());

    // Verify 0 distribution
    let distribution = result.unwrap();
    assert_eq!(distribution.get(0).unwrap(), 0);
}

#[test]
fn test_performance_with_fees() {
    let env = Env::default();
    env.mock_all_auctions();

    let strategy_id = 2u64;
    let initial = 10000i128;
    let current = 12000i128; // 20% gain
    let rewards = 0i128;
    let created_at = env.ledger().timestamp();

    let result = performance::calculate_performance(
        &env,
        strategy_id,
        initial,
        current,
        rewards,
        500, // 5% performance fee
        100, // 1% management fee
        created_at,
    );

    assert!(result.is_ok());
    let metrics = result.unwrap();

    // Gross return: 12000 - 10000 = 2000
    assert_eq!(metrics.total_return, 2000);

    // Performance fee should be on the 2000 gain: 5% of 2000 = 100
    assert_eq!(metrics.performance_fees, 100);
}
