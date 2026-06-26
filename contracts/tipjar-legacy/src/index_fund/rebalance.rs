//! Rebalancing logic — redistribute fund value according to target weights.

use soroban_sdk::Env;

use super::{get_creator_alloc, save_fund, set_creator_alloc};

/// Rebalance a fund: recalculate each creator's allocation based on current
/// total_value and target weights. Does not move tokens — just updates the
/// on-chain allocation records so deposits/withdrawals use the new targets.
///
/// Only the fund manager may call this.
pub fn rebalance(env: &Env, fund_id: u64, caller: &soroban_sdk::Address) {
    let mut fund = super::get_fund(env, fund_id).expect("Fund not found");

    if fund.manager != *caller {
        panic!("Only the fund manager can rebalance");
    }
    if !fund.active {
        panic!("Fund is not active");
    }

    let total_value = fund.total_value;

    // Recompute each creator's target allocation.
    for component in fund.components.iter() {
        let target = total_value * (component.weight_bps as i128) / 10_000;
        set_creator_alloc(env, fund_id, &component.creator, target);
    }

    fund.last_rebalanced = env.ledger().timestamp();
    save_fund(env, &fund);
}

/// Return the current allocation for each creator as a Vec of (creator, amount) pairs.
pub fn get_allocations(env: &Env, fund_id: u64) -> soroban_sdk::Vec<(soroban_sdk::Address, i128)> {
    let fund = super::get_fund(env, fund_id).expect("Fund not found");
    let mut result = soroban_sdk::Vec::new(env);
    for component in fund.components.iter() {
        let alloc = get_creator_alloc(env, fund_id, &component.creator);
        result.push_back((component.creator.clone(), alloc));
    }
    result
}
