//! Supply tracker for synthetic assets
//!
//! Monitors and records synthetic token supply, collateral amounts,
//! and collateralization ratios.

use super::events::{emit_collateral_updated, emit_supply_updated};
use super::types::SyntheticAsset;
use crate::{CoreError, CreditError, DataKey, SyntheticKey, TipJarError};
use soroban_sdk::Env;

/// Updates total supply after minting or redemption
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
/// - `delta`: Change in supply (positive for mint, negative for redeem)
///
/// # Requirements
/// - Validates: Requirements 3.5, 5.4, 6.1, 6.4, 6.9
pub fn update_supply(env: &Env, asset_id: u64, delta: i128) -> Result<(), TipJarError> {
    // Retrieve the synthetic asset
    let asset_key = DataKey::Synthetic(SyntheticKey::SyntheticAsset(asset_id));
    let mut asset: SyntheticAsset = env
        .storage()
        .persistent()
        .get(&asset_key)
        .ok_or(CreditError::SyntheticAssetNotFound)?;

    // Update the total supply
    asset.total_supply = asset
        .total_supply
        .checked_add(delta)
        .ok_or(CoreError::InvalidAmount)?;

    // Store the updated asset
    env.storage().persistent().set(&asset_key, &asset);

    // Emit SupplyUpdatedEvent
    emit_supply_updated(env, asset_id, asset.total_supply);

    Ok(())
}

/// Updates total collateral after operations
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
/// - `delta`: Change in collateral (positive for lock, negative for unlock)
///
/// # Requirements
/// - Validates: Requirements 3.6, 5.6, 6.2, 6.5, 6.10, 7.6
pub fn update_collateral(env: &Env, asset_id: u64, delta: i128) -> Result<(), TipJarError> {
    // Retrieve the synthetic asset
    let asset_key = DataKey::Synthetic(SyntheticKey::SyntheticAsset(asset_id));
    let mut asset: SyntheticAsset = env
        .storage()
        .persistent()
        .get(&asset_key)
        .ok_or(CreditError::SyntheticAssetNotFound)?;

    // Update the total collateral
    asset.total_collateral = asset
        .total_collateral
        .checked_add(delta)
        .ok_or(CoreError::InvalidAmount)?;

    // Store the updated asset
    env.storage().persistent().set(&asset_key, &asset);

    // Emit CollateralUpdatedEvent
    emit_collateral_updated(env, asset_id, asset.total_collateral);

    Ok(())
}

/// Calculates current collateralization ratio
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
///
/// # Returns
/// - Current collateralization ratio in basis points
///
/// # Formula
/// - ratio = (total_collateral / (total_supply * oracle_price)) * 10000
/// - When total_supply is zero, returns u32::MAX to indicate infinite collateralization
///
/// # Requirements
/// - Validates: Requirements 6.3, 6.8
pub fn get_collateralization_ratio(env: &Env, asset_id: u64) -> Result<u32, TipJarError> {
    // Retrieve the synthetic asset
    let asset_key = DataKey::Synthetic(SyntheticKey::SyntheticAsset(asset_id));
    let asset: SyntheticAsset = env
        .storage()
        .persistent()
        .get(&asset_key)
        .ok_or(CreditError::SyntheticAssetNotFound)?;

    // Handle division by zero when supply is zero
    if asset.total_supply == 0 {
        // When supply is zero, collateralization is effectively infinite
        return Ok(u32::MAX);
    }

    // Calculate the synthetic value: total_supply * oracle_price
    let synthetic_value = asset
        .total_supply
        .checked_mul(asset.oracle_price)
        .ok_or(CoreError::InvalidAmount)?;

    // Handle case where synthetic value is zero
    if synthetic_value == 0 {
        return Ok(u32::MAX);
    }

    // Calculate ratio: (total_collateral / synthetic_value) * 10000
    let ratio = asset
        .total_collateral
        .checked_mul(10000)
        .ok_or(CoreError::InvalidAmount)?
        .checked_div(synthetic_value)
        .ok_or(CoreError::InvalidAmount)?;

    // Convert to u32, capping at u32::MAX if overflow
    let ratio_u32 = if ratio > u32::MAX as i128 {
        u32::MAX
    } else if ratio < 0 {
        0
    } else {
        ratio as u32
    };

    Ok(ratio_u32)
}

/// Retrieves total supply for an asset
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
///
/// # Returns
/// - Total supply of synthetic tokens
///
/// # Requirements
/// - Validates: Requirements 6.6, 9.4
pub fn get_total_supply(env: &Env, asset_id: u64) -> Result<i128, TipJarError> {
    // Retrieve the synthetic asset
    let asset_key = DataKey::Synthetic(SyntheticKey::SyntheticAsset(asset_id));
    let asset: SyntheticAsset = env
        .storage()
        .persistent()
        .get(&asset_key)
        .ok_or(CreditError::SyntheticAssetNotFound)?;

    Ok(asset.total_supply)
}

/// Retrieves total collateral for an asset
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
///
/// # Returns
/// - Total collateral backing the synthetic asset
///
/// # Requirements
/// - Validates: Requirements 6.7, 9.4
pub fn get_total_collateral(env: &Env, asset_id: u64) -> Result<i128, TipJarError> {
    // Retrieve the synthetic asset
    let asset_key = DataKey::Synthetic(SyntheticKey::SyntheticAsset(asset_id));
    let asset: SyntheticAsset = env
        .storage()
        .persistent()
        .get(&asset_key)
        .ok_or(CreditError::SyntheticAssetNotFound)?;

    Ok(asset.total_collateral)
}
