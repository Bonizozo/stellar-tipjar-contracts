#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};
use tipjar::{
    stop_loss::{StopOrderKind, StopOrderStatus},
    TipJarContract, TipJarContractClient,
};

fn setup() -> (Env, TipJarContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin);
    client.add_token(&admin, &token_id);

    (env, client, admin, token_id)
}

fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
    soroban_sdk::token::StellarAssetClient::new(env, token).mint(to, &amount);
}

// ── Stop Loss ────────────────────────────────────────────────────────────────

#[test]
fn test_place_and_cancel_stop_loss() {
    let (env, client, _admin, token) = setup();
    let owner = Address::generate(&env);
    mint(&env, &token, &owner, 1_000_000);

    let order_id = client.sl_place_order(
        &owner,
        &token,
        &500_000i128,
        &900_000i128, // stop_price
        &0i128,       // limit_price (unused for StopLoss)
        &0i128,       // trail_amount (unused)
        &StopOrderKind::StopLoss,
        &1_000_000i128, // current_price
    );

    let order = client.sl_get_order(&order_id).unwrap();
    assert_eq!(order.status, StopOrderStatus::Active);
    assert_eq!(order.amount, 500_000);
    assert_eq!(order.stop_price, 900_000);

    // Cancel returns tokens
    client.sl_cancel_order(&owner, &order_id);
    let order = client.sl_get_order(&order_id).unwrap();
    assert_eq!(order.status, StopOrderStatus::Cancelled);

    let bal = soroban_sdk::token::Client::new(&env, &token).balance(&owner);
    assert_eq!(bal, 1_000_000); // fully refunded
}

#[test]
fn test_stop_loss_trigger_and_execute() {
    let (env, client, _admin, token) = setup();
    let owner = Address::generate(&env);
    mint(&env, &token, &owner, 1_000_000);

    let order_id = client.sl_place_order(
        &owner,
        &token,
        &500_000i128,
        &800_000i128, // stop_price
        &0i128,
        &0i128,
        &StopOrderKind::StopLoss,
        &1_000_000i128,
    );

    // Price above stop — not triggered
    let triggered = client.sl_check_trigger(&order_id, &900_000i128);
    assert!(!triggered);

    // Price at or below stop — triggered
    let triggered = client.sl_check_trigger(&order_id, &800_000i128);
    assert!(triggered);

    let order = client.sl_get_order(&order_id).unwrap();
    assert_eq!(order.status, StopOrderStatus::Triggered);

    // Execute the order
    client.sl_execute_order(&owner, &order_id, &800_000i128);
    let order = client.sl_get_order(&order_id).unwrap();
    assert_eq!(order.status, StopOrderStatus::Executed);

    // Tokens returned to owner
    let bal = soroban_sdk::token::Client::new(&env, &token).balance(&owner);
    assert_eq!(bal, 1_000_000);
}

#[test]
fn test_stop_limit_requires_limit_price() {
    let (env, client, _admin, token) = setup();
    let owner = Address::generate(&env);
    mint(&env, &token, &owner, 1_000_000);

    let order_id = client.sl_place_order(
        &owner,
        &token,
        &500_000i128,
        &800_000i128, // stop_price
        &750_000i128, // limit_price
        &0i128,
        &StopOrderKind::StopLimit,
        &1_000_000i128,
    );

    // Trigger the order
    client.sl_check_trigger(&order_id, &790_000i128);

    // Execution price below limit — should fail
    let result = std::panic::catch_unwind(|| {
        client.sl_execute_order(&owner, &order_id, &700_000i128);
    });
    assert!(result.is_err(), "Should panic when execution price < limit price");

    // Execution price meets limit — should succeed
    client.sl_execute_order(&owner, &order_id, &760_000i128);
    let order = client.sl_get_order(&order_id).unwrap();
    assert_eq!(order.status, StopOrderStatus::Executed);
}

#[test]
fn test_trailing_stop_adjusts_with_price() {
    let (env, client, _admin, token) = setup();
    let owner = Address::generate(&env);
    mint(&env, &token, &owner, 1_000_000);

    // Trail amount = 100_000; initial stop = 900_000 (peak 1_000_000 - trail)
    let order_id = client.sl_place_order(
        &owner,
        &token,
        &500_000i128,
        &900_000i128,   // initial stop_price
        &0i128,
        &100_000i128,   // trail_amount
        &StopOrderKind::TrailingStop,
        &1_000_000i128, // current_price (peak)
    );

    // Price rises to 1_200_000 — stop should trail up to 1_100_000
    client.sl_update_price(&order_id, &1_200_000i128);
    let order = client.sl_get_order(&order_id).unwrap();
    assert_eq!(order.stop_price, 1_100_000);
    assert_eq!(order.peak_price, 1_200_000);

    // Price drops to 1_050_000 — still above new stop, not triggered
    let triggered = client.sl_check_trigger(&order_id, &1_050_000i128);
    assert!(!triggered);

    // Price drops to 1_100_000 — exactly at stop, triggered
    let triggered = client.sl_check_trigger(&order_id, &1_100_000i128);
    assert!(triggered);
}

#[test]
fn test_get_owner_and_active_orders() {
    let (env, client, _admin, token) = setup();
    let owner = Address::generate(&env);
    mint(&env, &token, &owner, 3_000_000);

    let id0 = client.sl_place_order(
        &owner, &token, &1_000_000i128, &800_000i128, &0i128, &0i128,
        &StopOrderKind::StopLoss, &1_000_000i128,
    );
    let id1 = client.sl_place_order(
        &owner, &token, &1_000_000i128, &700_000i128, &0i128, &0i128,
        &StopOrderKind::StopLoss, &1_000_000i128,
    );

    let owner_orders = client.sl_get_owner_orders(&owner);
    assert_eq!(owner_orders.len(), 2);

    let active = client.sl_get_active_orders();
    assert!(active.contains(&id0));
    assert!(active.contains(&id1));

    // Cancel one; it should leave active list
    client.sl_cancel_order(&owner, &id0);
    let active = client.sl_get_active_orders();
    assert!(!active.contains(&id0));
    assert!(active.contains(&id1));
}
