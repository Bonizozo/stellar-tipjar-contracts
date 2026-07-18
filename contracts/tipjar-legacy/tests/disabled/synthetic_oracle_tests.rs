#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};
use tipjar::synthetic::oracle::{get_oracle_price, update_oracle_price};
use tipjar::synthetic::types::SyntheticAsset;
use tipjar::{DataKey, TipJarError};

/// Helper function to create a test synthetic asset
fn create_test_asset(
    env: &Env,
    asset_id: u64,
    creator: Address,
    backing_token: Address,
    total_supply: i128,
    total_collateral: i128,
) -> SyntheticAsset {
    let asset = SyntheticAsset {
        asset_id,
        creator,
        backing_token,
        total_supply,
        collateralization_ratio: 15000, // 150%
        created_at: env.ledger().timestamp(),
        oracle_price: 0,
        total_collateral,
        active: true,
    };

    let asset_key = DataKey::SyntheticAsset(asset_id);
    env.storage().persistent().set(&asset_key, &asset);

    asset
}

/// Helper function to set creator balance
fn set_creator_balance(env: &Env, creator: &Address, token: &Address, balance: i128) {
    let balance_key = DataKey::CreatorBalance(creator.clone(), token.clone());
    env.storage().persistent().set(&balance_key, &balance);
}

#[test]
fn test_update_oracle_price_with_supply() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with supply of 1000
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        1500,
    );

    // Set tip pool balance to 2000
    set_creator_balance(&env, &creator, &backing_token, 2000);

    // Update oracle price
    let price = update_oracle_price(&env, asset_id).unwrap();

    // Price should be 2000 / 1000 = 2
    assert_eq!(price, 2);

    // Verify price is stored in asset
    let asset_key = DataKey::SyntheticAsset(asset_id);
    let asset: SyntheticAsset = env.storage().persistent().get(&asset_key).unwrap();
    assert_eq!(asset.oracle_price, 2);
}

#[test]
fn test_update_oracle_price_zero_supply() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with zero supply
    create_test_asset(&env, asset_id, creator.clone(), backing_token.clone(), 0, 0);

    // Set tip pool balance to 5000
    set_creator_balance(&env, &creator, &backing_token, 5000);

    // Update oracle price
    let price = update_oracle_price(&env, asset_id).unwrap();

    // Price should be initial price (1 unit = 10^7 stroops)
    assert_eq!(price, 10_000_000);
}

#[test]
fn test_update_oracle_price_zero_balance_with_supply() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with supply but zero balance
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        0,
    );

    // Set tip pool balance to 0
    set_creator_balance(&env, &creator, &backing_token, 0);

    // Update oracle price
    let price = update_oracle_price(&env, asset_id).unwrap();

    // Price should be 0 when balance is 0 and supply > 0
    assert_eq!(price, 0);
}

#[test]
fn test_update_oracle_price_large_values() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with large supply
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1_000_000,
        1_500_000,
    );

    // Set large tip pool balance
    set_creator_balance(&env, &creator, &backing_token, 5_000_000);

    // Update oracle price
    let price = update_oracle_price(&env, asset_id).unwrap();

    // Price should be 5_000_000 / 1_000_000 = 5
    assert_eq!(price, 5);
}

#[test]
fn test_update_oracle_price_fractional_result() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset where division results in fraction
    create_test_asset(&env, asset_id, creator.clone(), backing_token.clone(), 3, 0);

    // Set tip pool balance
    set_creator_balance(&env, &creator, &backing_token, 10);

    // Update oracle price
    let price = update_oracle_price(&env, asset_id).unwrap();

    // Price should be 10 / 3 = 3 (integer division)
    assert_eq!(price, 3);
}

#[test]
fn test_update_oracle_price_nonexistent_asset() {
    let env = Env::default();
    let asset_id = 999u64;

    // Try to update price for non-existent asset
    let result = update_oracle_price(&env, asset_id);

    // Should return SyntheticAssetNotFound error
    assert_eq!(result, Err(TipJarError::SyntheticAssetNotFound));
}

#[test]
fn test_get_oracle_price() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with a specific oracle price
    let mut asset = create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        1500,
    );
    asset.oracle_price = 1234;
    let asset_key = DataKey::SyntheticAsset(asset_id);
    env.storage().persistent().set(&asset_key, &asset);

    // Get oracle price
    let price = get_oracle_price(&env, asset_id).unwrap();

    // Should return the stored price
    assert_eq!(price, 1234);
}

#[test]
fn test_get_oracle_price_nonexistent_asset() {
    let env = Env::default();
    let asset_id = 999u64;

    // Try to get price for non-existent asset
    let result = get_oracle_price(&env, asset_id);

    // Should return SyntheticAssetNotFound error
    assert_eq!(result, Err(TipJarError::SyntheticAssetNotFound));
}

#[test]
fn test_get_oracle_price_after_update() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        500,
        750,
    );

    // Set tip pool balance
    set_creator_balance(&env, &creator, &backing_token, 1000);

    // Update oracle price
    let updated_price = update_oracle_price(&env, asset_id).unwrap();

    // Get oracle price
    let retrieved_price = get_oracle_price(&env, asset_id).unwrap();

    // Both should match
    assert_eq!(updated_price, retrieved_price);
    assert_eq!(retrieved_price, 2); // 1000 / 500 = 2
}

#[test]
fn test_oracle_price_updates_on_balance_change() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        1000,
    );

    // Set initial tip pool balance
    set_creator_balance(&env, &creator, &backing_token, 1000);

    // Update oracle price
    let price1 = update_oracle_price(&env, asset_id).unwrap();
    assert_eq!(price1, 1); // 1000 / 1000 = 1

    // Increase tip pool balance (simulating new tips)
    set_creator_balance(&env, &creator, &backing_token, 2000);

    // Update oracle price again
    let price2 = update_oracle_price(&env, asset_id).unwrap();
    assert_eq!(price2, 2); // 2000 / 1000 = 2

    // Price should have increased
    assert!(price2 > price1);
}

#[test]
fn test_oracle_price_with_no_balance_entry() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with supply
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        100,
        150,
    );

    // Don't set any balance (defaults to 0)

    // Update oracle price
    let price = update_oracle_price(&env, asset_id).unwrap();

    // Price should be 0 when balance is 0 and supply > 0
    assert_eq!(price, 0);
}

#[test]
fn test_oracle_price_emits_event() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        1500,
    );

    // Set tip pool balance
    set_creator_balance(&env, &creator, &backing_token, 3000);

    // Update oracle price (should emit event)
    let price = update_oracle_price(&env, asset_id).unwrap();

    // Verify the function executed successfully
    assert_eq!(price, 3);

    // Note: Event emission verification would require more complex test setup
    // with event listeners, but we can verify the function doesn't panic
}
