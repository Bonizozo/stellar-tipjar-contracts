#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Env as _},
    vec as svec, Address, Env,
};

use tipjar::portfolio::{
    self, AssetAdjustment, AssetTarget, PortfolioError,
};

// ── helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    (env, owner)
}

fn two_asset_targets(env: &Env) -> soroban_sdk::Vec<AssetTarget> {
    let t1 = Address::generate(env);
    let t2 = Address::generate(env);
    svec![
        env,
        AssetTarget { token: t1, target_bps: 6000, min_bps: 5000, max_bps: 7000 },
        AssetTarget { token: t2, target_bps: 4000, min_bps: 3000, max_bps: 5000 },
    ]
}

// ── creation tests ────────────────────────────────────────────────────────────

#[test]
fn test_create_portfolio_success() {
    let (env, owner) = setup();
    let assets = two_asset_targets(&env);

    let pid = portfolio::create_portfolio(&env, owner.clone(), assets, 200, 86_400, 10)
        .expect("create_portfolio should succeed");

    let p = portfolio::get_portfolio(&env, pid).expect("portfolio should exist");
    assert_eq!(p.portfolio_id, pid);
    assert_eq!(p.owner, owner);
    assert_eq!(p.assets.len(), 2);
    assert!(p.active);
    assert_eq!(p.rebalance_count, 0);
}

#[test]
#[should_panic]
fn test_create_portfolio_invalid_weights_panics() {
    let (env, owner) = setup();
    let t1 = Address::generate(&env);
    // Weights sum to 9000, not 10000
    let assets = svec![
        &env,
        AssetTarget { token: t1, target_bps: 9000, min_bps: 0, max_bps: 10000 },
    ];
    portfolio::create_portfolio(&env, owner, assets, 200, 86_400, 10).unwrap();
}

#[test]
#[should_panic]
fn test_create_portfolio_empty_assets_panics() {
    let (env, owner) = setup();
    let assets: soroban_sdk::Vec<AssetTarget> = soroban_sdk::Vec::new(&env);
    portfolio::create_portfolio(&env, owner, assets, 200, 86_400, 10).unwrap();
}

#[test]
fn test_owner_portfolios_tracked() {
    let (env, owner) = setup();

    portfolio::create_portfolio(&env, owner.clone(), two_asset_targets(&env), 200, 86_400, 10)
        .unwrap();
    portfolio::create_portfolio(&env, owner.clone(), two_asset_targets(&env), 300, 3_600, 5)
        .unwrap();

    let ids = portfolio::get_owner_portfolios(&env, &owner);
    assert_eq!(ids.len(), 2);
}

// ── optimal allocation tests ──────────────────────────────────────────────────

#[test]
fn test_calculate_optimal_allocations_balanced() {
    let (env, owner) = setup();
    let assets = two_asset_targets(&env);
    let pid =
        portfolio::create_portfolio(&env, owner, assets, 200, 86_400, 0).unwrap();

    // Current: 600 / 400 — already at target, delta should be 0
    let current = svec![&env, 600i128, 400i128];
    let adjustments =
        portfolio::calculate_optimal_allocations(&env, pid, &current).unwrap();

    assert_eq!(adjustments.len(), 2);
    assert_eq!(adjustments.get(0).unwrap().delta, 0);
    assert_eq!(adjustments.get(1).unwrap().delta, 0);
}

#[test]
fn test_calculate_optimal_allocations_drift() {
    let (env, owner) = setup();
    let assets = two_asset_targets(&env);
    let pid =
        portfolio::create_portfolio(&env, owner, assets, 200, 86_400, 0).unwrap();

    // Current: 500 / 500 — target is 600/400, so asset0 needs +100, asset1 needs -100
    let current = svec![&env, 500i128, 500i128];
    let adjustments =
        portfolio::calculate_optimal_allocations(&env, pid, &current).unwrap();

    assert_eq!(adjustments.get(0).unwrap().delta, 100);
    assert_eq!(adjustments.get(1).unwrap().delta, -100);
}

#[test]
fn test_calculate_optimal_allocations_with_tx_cost() {
    let (env, owner) = setup();
    let assets = two_asset_targets(&env);
    // tx_cost_bps = 100 (1%)
    let pid =
        portfolio::create_portfolio(&env, owner, assets, 200, 86_400, 100).unwrap();

    let current = svec![&env, 500i128, 500i128];
    let adjustments =
        portfolio::calculate_optimal_allocations(&env, pid, &current).unwrap();

    // delta for asset0 = 100, tx_cost = 1% of 100 = 1
    assert_eq!(adjustments.get(0).unwrap().tx_cost, 1);
    assert_eq!(adjustments.get(1).unwrap().tx_cost, 1);
}

// ── needs_rebalance tests ─────────────────────────────────────────────────────

#[test]
fn test_needs_rebalance_too_frequent() {
    let (env, owner) = setup();
    let assets = two_asset_targets(&env);
    // frequency = 1 day; ledger starts at 0 so it's immediately too frequent
    let pid =
        portfolio::create_portfolio(&env, owner, assets, 200, 86_400, 0).unwrap();

    let current = svec![&env, 500i128, 500i128];
    let result = portfolio::needs_rebalance(&env, pid, &current);
    assert!(matches!(result, Err(PortfolioError::RebalanceTooFrequent)));
}

#[test]
fn test_needs_rebalance_after_frequency_elapsed() {
    let (env, owner) = setup();
    let assets = two_asset_targets(&env);
    // frequency = 0 seconds so it's always due
    let pid =
        portfolio::create_portfolio(&env, owner, assets, 200, 0, 0).unwrap();

    // Drift: 500/500 vs target 600/400 → 10% drift > 2% tolerance
    let current = svec![&env, 500i128, 500i128];
    let result = portfolio::needs_rebalance(&env, pid, &current);
    assert_eq!(result, Ok(true));
}

#[test]
fn test_needs_rebalance_within_tolerance() {
    let (env, owner) = setup();
    let assets = two_asset_targets(&env);
    // frequency = 0, tolerance = 2000 bps (20%) — drift is small
    let pid =
        portfolio::create_portfolio(&env, owner, assets, 2000, 0, 0).unwrap();

    // Current: 590/410 — drift from 600/400 is ~1.7%, within 20% tolerance
    let current = svec![&env, 590i128, 410i128];
    let result = portfolio::needs_rebalance(&env, pid, &current);
    assert_eq!(result, Ok(false));
}

// ── execute_rebalance tests ───────────────────────────────────────────────────

#[test]
fn test_execute_rebalance_records_history() {
    let (env, owner) = setup();
    let assets = two_asset_targets(&env);
    let pid =
        portfolio::create_portfolio(&env, owner.clone(), assets, 200, 0, 0).unwrap();

    let current = svec![&env, 500i128, 500i128];
    let adjustments =
        portfolio::calculate_optimal_allocations(&env, pid, &current).unwrap();

    let entry = portfolio::execute_rebalance(&env, owner.clone(), pid, &adjustments)
        .expect("execute_rebalance should succeed");

    assert_eq!(entry.portfolio_id, pid);
    assert_eq!(entry.assets_adjusted, 2);
    assert_eq!(entry.total_adjusted, 200); // |100| + |100|

    // Portfolio rebalance_count should be 1
    let p = portfolio::get_portfolio(&env, pid).unwrap();
    assert_eq!(p.rebalance_count, 1);

    // History count should be 1
    assert_eq!(portfolio::get_history_count(&env, pid), 1);
}

#[test]
fn test_execute_rebalance_history_retrievable() {
    let (env, owner) = setup();
    let assets = two_asset_targets(&env);
    let pid =
        portfolio::create_portfolio(&env, owner.clone(), assets, 200, 0, 0).unwrap();

    let current = svec![&env, 500i128, 500i128];
    let adjustments =
        portfolio::calculate_optimal_allocations(&env, pid, &current).unwrap();

    portfolio::execute_rebalance(&env, owner, pid, &adjustments).unwrap();

    let entry = portfolio::get_history_entry(&env, pid, 0).expect("history entry should exist");
    assert_eq!(entry.entry_index, 0);
    assert_eq!(entry.portfolio_id, pid);
}

#[test]
#[should_panic]
fn test_execute_rebalance_wrong_owner_panics() {
    let (env, owner) = setup();
    let other = Address::generate(&env);
    let assets = two_asset_targets(&env);
    let pid =
        portfolio::create_portfolio(&env, owner, assets, 200, 0, 0).unwrap();

    let adjustments: soroban_sdk::Vec<AssetAdjustment> = soroban_sdk::Vec::new(&env);
    portfolio::execute_rebalance(&env, other, pid, &adjustments).unwrap();
}

// ── query tests ───────────────────────────────────────────────────────────────

#[test]
fn test_get_nonexistent_portfolio_returns_none() {
    let (env, _) = setup();
    assert!(portfolio::get_portfolio(&env, 999).is_none());
}

#[test]
fn test_get_history_entry_nonexistent_returns_none() {
    let (env, _) = setup();
    assert!(portfolio::get_history_entry(&env, 0, 0).is_none());
}

#[test]
fn test_history_count_zero_initially() {
    let (env, owner) = setup();
    let assets = two_asset_targets(&env);
    let pid =
        portfolio::create_portfolio(&env, owner, assets, 200, 86_400, 0).unwrap();
    assert_eq!(portfolio::get_history_count(&env, pid), 0);
}
