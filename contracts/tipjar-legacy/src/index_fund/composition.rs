//! Index composition management — define and validate creator baskets.

use soroban_sdk::{Address, Env, String, Vec};

use super::{next_fund_id, save_fund, IndexComponent, IndexFund};

/// Validate that component weights sum to exactly 10_000 bps.
pub fn validate_weights(components: &Vec<IndexComponent>) -> bool {
    if components.is_empty() {
        return false;
    }
    let sum: u32 = components.iter().map(|c| c.weight_bps).sum();
    sum == 10_000
}

/// Create a new index fund with the given composition.
/// Panics if weights don't sum to 10_000 or if fewer than 2 creators are provided.
pub fn create_index_fund(
    env: &Env,
    manager: &Address,
    token: &Address,
    name: String,
    components: Vec<IndexComponent>,
) -> u64 {
    if components.len() < 2 {
        panic!("Index fund requires at least 2 creators");
    }
    if !validate_weights(&components) {
        panic!("Component weights must sum to 10000 bps");
    }

    let id = next_fund_id(env);
    let now = env.ledger().timestamp();

    let fund = IndexFund {
        id,
        name,
        manager: manager.clone(),
        token: token.clone(),
        components,
        total_shares: 0,
        total_value: 0,
        created_at: now,
        last_rebalanced: now,
        active: true,
    };

    save_fund(env, &fund);
    id
}

/// Update the composition of an existing fund (manager only).
/// Resets allocations proportionally based on new weights.
pub fn update_composition(
    env: &Env,
    fund_id: u64,
    caller: &Address,
    new_components: Vec<IndexComponent>,
) {
    let mut fund = super::get_fund(env, fund_id).expect("Fund not found");

    if fund.manager != *caller {
        panic!("Only the fund manager can update composition");
    }
    if new_components.len() < 2 {
        panic!("Index fund requires at least 2 creators");
    }
    if !validate_weights(&new_components) {
        panic!("Component weights must sum to 10000 bps");
    }

    fund.components = new_components;
    save_fund(env, &fund);
}

/// Get the list of creators in a fund.
pub fn get_creators(env: &Env, fund_id: u64) -> Vec<Address> {
    let fund = super::get_fund(env, fund_id).expect("Fund not found");
    let mut creators = Vec::new(env);
    for c in fund.components.iter() {
        creators.push_back(c.creator.clone());
    }
    creators
}
