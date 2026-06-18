//! Price oracle for synthetic assets
//!
//! Calculates real-time valuations of synthetic assets based on tip pool
//! performance metrics.

use super::events::emit_price_updated;
use super::types::SyntheticAsset;
use crate::{CreditError, DataKey, SyntheticKey, TipJarError};
use soroban_sdk::Env;

/// Calculates and updates the oracle price for a synthetic asset
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
///
/// # Returns
/// - Updated oracle price
///
/// # Formula
/// - If total_supply > 0: price = tip_pool_balance / total_supply
/// - If total_supply == 0: price = 1 unit of backing token (10^7 stroops)
/// - If tip_pool_balance == 0 and total_supply > 0: price = 0
///
/// # Requirements
/// - Validates: Requirements 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7
pub fn update_oracle_price(env: &Env, asset_id: u64) -> Result<i128, TipJarError> {
    // Retrieve the synthetic asset
    let asset_key = DataKey::Synthetic(SyntheticKey::SyntheticAsset(asset_id));
    let mut asset: SyntheticAsset = env
        .storage()
        .persistent()
        .get(&asset_key)
        .ok_or(CreditError::SyntheticAssetNotFound)?;

    // Get the tip pool balance for the creator and backing token
    let balance_key = DataKey::CreatorBalance(asset.creator.clone(), asset.backing_token.clone());
    let tip_pool_balance: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0);

    // Calculate the oracle price based on the formula
    let new_price = if asset.total_supply > 0 {
        if tip_pool_balance == 0 {
            // Zero balance with non-zero supply returns zero price
            0
        } else {
            // Price = tip_pool_balance / total_supply
            tip_pool_balance
                .checked_div(asset.total_supply)
                .unwrap_or(0)
        }
    } else {
        // When supply is zero, return initial price (1 unit of backing token)
        // In Stellar/Soroban, 1 unit = 10^7 stroops
        10_000_000
    };

    // Store the updated price in the synthetic asset record
    asset.oracle_price = new_price;
    env.storage().persistent().set(&asset_key, &asset);

    // Emit PriceUpdatedEvent
    emit_price_updated(env, asset_id, new_price);

    Ok(new_price)
}

/// Retrieves the current oracle price without updating
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
///
/// # Returns
/// - Current oracle price
///
/// # Errors
/// - `SyntheticAssetNotFound`: Asset does not exist
///
/// # Requirements
/// - Validates: Requirements 4.1, 9.3
pub fn get_oracle_price(env: &Env, asset_id: u64) -> Result<i128, TipJarError> {
    // Retrieve the synthetic asset
    let asset_key = DataKey::Synthetic(SyntheticKey::SyntheticAsset(asset_id));
    let asset: SyntheticAsset = env
        .storage()
        .persistent()
        .get(&asset_key)
        .ok_or(CreditError::SyntheticAssetNotFound)?;

    // Return the current oracle price
    Ok(asset.oracle_price)
}
