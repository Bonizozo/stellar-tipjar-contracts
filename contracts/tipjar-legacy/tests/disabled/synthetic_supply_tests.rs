#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};
use tipjar::synthetic::supply::{
    get_collateralization_ratio, get_total_collateral, get_total_supply, update_collateral,
    update_supply,
};
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
    oracle_price: i128,
) -> SyntheticAsset {
    let asset = SyntheticAsset {
        asset_id,
        creator,
        backing_token,
        total_supply,
        collateralization_ratio: 15000, // 150%
        created_at: env.ledger().timestamp(),
        oracle_price,
        total_collateral,
        active: true,
    };

    let asset_key = DataKey::SyntheticAsset(asset_id);
    env.storage().persistent().set(&asset_key, &asset);

    asset
}

#[test]
fn test_update_supply_positive_delta() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with initial supply of 1000
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        1500,
        10,
    );

    // Update supply by +500 (minting)
    update_supply(&env, asset_id, 500).unwrap();

    // Verify supply increased
    let new_supply = get_total_supply(&env, asset_id).unwrap();
    assert_eq!(new_supply, 1500);
}

#[test]
fn test_update_supply_negative_delta() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with initial supply of 1000
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        1500,
        10,
    );

    // Update supply by -300 (redemption)
    update_supply(&env, asset_id, -300).unwrap();

    // Verify supply decreased
    let new_supply = get_total_supply(&env, asset_id).unwrap();
    assert_eq!(new_supply, 700);
}

#[test]
fn test_update_supply_nonexistent_asset() {
    let env = Env::default();
    let asset_id = 999u64;

    // Try to update supply for non-existent asset
    let result = update_supply(&env, asset_id, 100);

    // Should return SyntheticAssetNotFound error
    assert_eq!(result, Err(TipJarError::SyntheticAssetNotFound));
}

#[test]
fn test_update_collateral_positive_delta() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with initial collateral of 1500
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        1500,
        10,
    );

    // Update collateral by +500 (locking)
    update_collateral(&env, asset_id, 500).unwrap();

    // Verify collateral increased
    let new_collateral = get_total_collateral(&env, asset_id).unwrap();
    assert_eq!(new_collateral, 2000);
}

#[test]
fn test_update_collateral_negative_delta() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with initial collateral of 1500
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        1500,
        10,
    );

    // Update collateral by -300 (unlocking)
    update_collateral(&env, asset_id, -300).unwrap();

    // Verify collateral decreased
    let new_collateral = get_total_collateral(&env, asset_id).unwrap();
    assert_eq!(new_collateral, 1200);
}

#[test]
fn test_update_collateral_nonexistent_asset() {
    let env = Env::default();
    let asset_id = 999u64;

    // Try to update collateral for non-existent asset
    let result = update_collateral(&env, asset_id, 100);

    // Should return SyntheticAssetNotFound error
    assert_eq!(result, Err(TipJarError::SyntheticAssetNotFound));
}

#[test]
fn test_get_collateralization_ratio_normal() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with:
    // - total_supply: 1000
    // - oracle_price: 10
    // - total_collateral: 15000
    // Expected ratio: (15000 / (1000 * 10)) * 10000 = 15000 bps (150%)
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        15000,
        10,
    );

    let ratio = get_collateralization_ratio(&env, asset_id).unwrap();
    assert_eq!(ratio, 15000);
}

#[test]
fn test_get_collateralization_ratio_zero_supply() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with zero supply
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        0,
        1000,
        10,
    );

    // When supply is zero, ratio should be u32::MAX (infinite)
    let ratio = get_collateralization_ratio(&env, asset_id).unwrap();
    assert_eq!(ratio, u32::MAX);
}

#[test]
fn test_get_collateralization_ratio_zero_price() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with zero oracle price
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        1500,
        0,
    );

    // When price is zero, synthetic value is zero, ratio should be u32::MAX
    let ratio = get_collateralization_ratio(&env, asset_id).unwrap();
    assert_eq!(ratio, u32::MAX);
}

#[test]
fn test_get_collateralization_ratio_undercollateralized() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with:
    // - total_supply: 1000
    // - oracle_price: 10
    // - total_collateral: 8000
    // Expected ratio: (8000 / (1000 * 10)) * 10000 = 8000 bps (80%)
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        8000,
        10,
    );

    let ratio = get_collateralization_ratio(&env, asset_id).unwrap();
    assert_eq!(ratio, 8000);
}

#[test]
fn test_get_collateralization_ratio_overcollateralized() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with:
    // - total_supply: 1000
    // - oracle_price: 10
    // - total_collateral: 50000
    // Expected ratio: (50000 / (1000 * 10)) * 10000 = 50000 bps (500%)
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        50000,
        10,
    );

    let ratio = get_collateralization_ratio(&env, asset_id).unwrap();
    assert_eq!(ratio, 50000);
}

#[test]
fn test_get_collateralization_ratio_nonexistent_asset() {
    let env = Env::default();
    let asset_id = 999u64;

    // Try to get ratio for non-existent asset
    let result = get_collateralization_ratio(&env, asset_id);

    // Should return SyntheticAssetNotFound error
    assert_eq!(result, Err(TipJarError::SyntheticAssetNotFound));
}

#[test]
fn test_get_total_supply() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with specific supply
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        12345,
        15000,
        10,
    );

    let supply = get_total_supply(&env, asset_id).unwrap();
    assert_eq!(supply, 12345);
}

#[test]
fn test_get_total_supply_nonexistent_asset() {
    let env = Env::default();
    let asset_id = 999u64;

    // Try to get supply for non-existent asset
    let result = get_total_supply(&env, asset_id);

    // Should return SyntheticAssetNotFound error
    assert_eq!(result, Err(TipJarError::SyntheticAssetNotFound));
}

#[test]
fn test_get_total_collateral() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with specific collateral
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        67890,
        10,
    );

    let collateral = get_total_collateral(&env, asset_id).unwrap();
    assert_eq!(collateral, 67890);
}

#[test]
fn test_get_total_collateral_nonexistent_asset() {
    let env = Env::default();
    let asset_id = 999u64;

    // Try to get collateral for non-existent asset
    let result = get_total_collateral(&env, asset_id);

    // Should return SyntheticAssetNotFound error
    assert_eq!(result, Err(TipJarError::SyntheticAssetNotFound));
}

#[test]
fn test_supply_tracking_invariant() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with initial supply of 1000
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        1500,
        10,
    );

    // Perform multiple operations
    update_supply(&env, asset_id, 500).unwrap(); // +500 -> 1500
    update_supply(&env, asset_id, 300).unwrap(); // +300 -> 1800
    update_supply(&env, asset_id, -200).unwrap(); // -200 -> 1600
    update_supply(&env, asset_id, -100).unwrap(); // -100 -> 1500

    // Verify final supply
    let final_supply = get_total_supply(&env, asset_id).unwrap();
    assert_eq!(final_supply, 1500);
}

#[test]
fn test_collateral_tracking_invariant() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with initial collateral of 1500
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        1500,
        10,
    );

    // Perform multiple operations
    update_collateral(&env, asset_id, 500).unwrap(); // +500 -> 2000
    update_collateral(&env, asset_id, 300).unwrap(); // +300 -> 2300
    update_collateral(&env, asset_id, -200).unwrap(); // -200 -> 2100
    update_collateral(&env, asset_id, -100).unwrap(); // -100 -> 2000

    // Verify final collateral
    let final_collateral = get_total_collateral(&env, asset_id).unwrap();
    assert_eq!(final_collateral, 2000);
}

#[test]
fn test_collateralization_ratio_after_supply_change() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with 150% collateralization
    // supply: 1000, price: 10, collateral: 15000
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        15000,
        10,
    );

    let initial_ratio = get_collateralization_ratio(&env, asset_id).unwrap();
    assert_eq!(initial_ratio, 15000);

    // Increase supply (minting) without adding collateral
    update_supply(&env, asset_id, 500).unwrap(); // supply now 1500

    // Ratio should decrease: (15000 / (1500 * 10)) * 10000 = 10000 bps (100%)
    let new_ratio = get_collateralization_ratio(&env, asset_id).unwrap();
    assert_eq!(new_ratio, 10000);
}

#[test]
fn test_collateralization_ratio_after_collateral_change() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let backing_token = Address::generate(&env);
    let asset_id = 1u64;

    // Create asset with 150% collateralization
    // supply: 1000, price: 10, collateral: 15000
    create_test_asset(
        &env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        1000,
        15000,
        10,
    );

    let initial_ratio = get_collateralization_ratio(&env, asset_id).unwrap();
    assert_eq!(initial_ratio, 15000);

    // Add collateral
    update_collateral(&env, asset_id, 5000).unwrap(); // collateral now 20000

    // Ratio should increase: (20000 / (1000 * 10)) * 10000 = 20000 bps (200%)
    let new_ratio = get_collateralization_ratio(&env, asset_id).unwrap();
    assert_eq!(new_ratio, 20000);
}
