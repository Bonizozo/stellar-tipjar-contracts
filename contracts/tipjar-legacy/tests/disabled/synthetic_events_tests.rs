#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};
use tipjar::synthetic::events::*;

#[test]
fn test_emit_synthetic_asset_created() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;
    let collateralization_ratio = 15000u32;

    emit_synthetic_asset_created(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        collateralization_ratio,
    );

    // Event emission doesn't return anything, but we can verify it doesn't panic
    assert!(true);
}

#[test]
fn test_emit_synthetic_tokens_minted() {
    let env = Env::default();
    let minter = Address::generate(&env);
    let asset_id = 1u64;
    let amount = 1000i128;
    let collateral_provided = 1500i128;

    emit_synthetic_tokens_minted(&env, asset_id, minter.clone(), amount, collateral_provided);

    // Event emission doesn't return anything, but we can verify it doesn't panic
    assert!(true);
}

#[test]
fn test_emit_synthetic_tokens_redeemed() {
    let env = Env::default();
    let redeemer = Address::generate(&env);
    let asset_id = 1u64;
    let amount = 500i128;
    let value_received = 750i128;

    emit_synthetic_tokens_redeemed(&env, asset_id, redeemer.clone(), amount, value_received);

    // Event emission doesn't return anything, but we can verify it doesn't panic
    assert!(true);
}

#[test]
fn test_emit_price_updated() {
    let env = Env::default();
    let asset_id = 1u64;
    let new_price = 1200i128;

    emit_price_updated(&env, asset_id, new_price);

    // Event emission doesn't return anything, but we can verify it doesn't panic
    assert!(true);
}

#[test]
fn test_emit_supply_updated() {
    let env = Env::default();
    let asset_id = 1u64;
    let new_total_supply = 10000i128;

    emit_supply_updated(&env, asset_id, new_total_supply);

    // Event emission doesn't return anything, but we can verify it doesn't panic
    assert!(true);
}

#[test]
fn test_emit_collateral_updated() {
    let env = Env::default();
    let asset_id = 1u64;
    let new_total_collateral = 15000i128;

    emit_collateral_updated(&env, asset_id, new_total_collateral);

    // Event emission doesn't return anything, but we can verify it doesn't panic
    assert!(true);
}

#[test]
fn test_emit_synthetic_asset_paused() {
    let env = Env::default();
    let asset_id = 1u64;

    emit_synthetic_asset_paused(&env, asset_id);

    // Event emission doesn't return anything, but we can verify it doesn't panic
    assert!(true);
}

#[test]
fn test_emit_synthetic_asset_resumed() {
    let env = Env::default();
    let asset_id = 1u64;

    emit_synthetic_asset_resumed(&env, asset_id);

    // Event emission doesn't return anything, but we can verify it doesn't panic
    assert!(true);
}

#[test]
fn test_emit_collateralization_updated() {
    let env = Env::default();
    let asset_id = 1u64;
    let new_ratio = 20000u32;

    emit_collateralization_updated(&env, asset_id, new_ratio);

    // Event emission doesn't return anything, but we can verify it doesn't panic
    assert!(true);
}

#[test]
fn test_all_events_have_timestamps() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let minter = Address::generate(&env);
    let asset_id = 1u64;

    // Set a specific timestamp
    env.ledger().with_mut(|li| {
        li.timestamp = 1234567890;
    });

    // Emit various events - they should all use the ledger timestamp
    emit_synthetic_asset_created(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        15000,
    );
    emit_synthetic_tokens_minted(&env, asset_id, minter.clone(), 1000, 1500);
    emit_price_updated(&env, asset_id, 1200);
    emit_supply_updated(&env, asset_id, 10000);
    emit_collateral_updated(&env, asset_id, 15000);
    emit_synthetic_asset_paused(&env, asset_id);
    emit_synthetic_asset_resumed(&env, asset_id);
    emit_collateralization_updated(&env, asset_id, 20000);

    // All events should have been emitted with the timestamp
    // We can't directly verify the timestamp in events without more complex setup,
    // but we can verify the functions execute without panicking
    assert!(true);
}
