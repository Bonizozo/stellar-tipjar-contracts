//! Query functions for synthetic assets
//!
//! Provides read-only access to synthetic asset information for users
//! to make informed decisions about minting and redemption.

use super::minting::calculate_required_collateral;
use super::oracle::get_oracle_price;
use super::redemption::calculate_redemption_value;
use super::supply::{get_collateralization_ratio, get_total_collateral, get_total_supply};
use super::types::SyntheticAsset;
use crate::{CreditError, DataKey, SyntheticKey, TipJarError};
use soroban_sdk::{Address, Env, Vec};

/// Retrieves synthetic asset details by asset identifier
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
///
/// # Returns
/// - Complete synthetic asset record
///
/// # Requirements
/// - Validates: Requirements 9.1, 9.10
pub fn get_synthetic_asset(env: &Env, asset_id: u64) -> Result<SyntheticAsset, TipJarError> {
    let asset_key = DataKey::Synthetic(SyntheticKey::SyntheticAsset(asset_id));
    let asset: SyntheticAsset = env
        .storage()
        .persistent()
        .get(&asset_key)
        .ok_or(CreditError::SyntheticAssetNotFound)?;

    Ok(asset)
}

/// Retrieves all synthetic assets for a creator
///
/// # Parameters
/// - `env`: Soroban environment
/// - `creator`: Creator address
///
/// # Returns
/// - Vector of asset identifiers for the creator
///
/// # Requirements
/// - Validates: Requirements 9.2
pub fn get_creator_synthetic_assets(env: &Env, creator: &Address) -> Vec<u64> {
    let creator_assets_key =
        DataKey::Synthetic(SyntheticKey::CreatorSyntheticAssets(creator.clone()));
    env.storage()
        .persistent()
        .get(&creator_assets_key)
        .unwrap_or(Vec::new(env))
}

/// Retrieves synthetic token balance for holder and asset
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
/// - `holder`: Address of the token holder
///
/// # Returns
/// - Balance of synthetic tokens (0 if no balance exists)
///
/// # Requirements
/// - Validates: Requirements 9.8
pub fn get_holder_balance(env: &Env, asset_id: u64, holder: &Address) -> i128 {
    let balance_key = DataKey::Synthetic(SyntheticKey::SyntheticBalance(holder.clone(), asset_id));
    env.storage().persistent().get(&balance_key).unwrap_or(0)
}

/// Retrieves the current oracle price for a synthetic asset
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
///
/// # Returns
/// - Current oracle price
///
/// # Requirements
/// - Validates: Requirements 9.3
pub fn get_synthetic_oracle_price(env: &Env, asset_id: u64) -> Result<i128, TipJarError> {
    get_oracle_price(env, asset_id)
}

/// Retrieves the total supply for a synthetic asset
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
///
/// # Returns
/// - Total supply of synthetic tokens
///
/// # Requirements
/// - Validates: Requirements 9.4
pub fn get_synthetic_total_supply(env: &Env, asset_id: u64) -> Result<i128, TipJarError> {
    get_total_supply(env, asset_id)
}

/// Retrieves the total collateral for a synthetic asset
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
///
/// # Returns
/// - Total collateral backing the synthetic asset
///
/// # Requirements
/// - Validates: Requirements 9.4
pub fn get_synthetic_total_collateral(env: &Env, asset_id: u64) -> Result<i128, TipJarError> {
    get_total_collateral(env, asset_id)
}

/// Retrieves the collateralization ratio for a synthetic asset
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
///
/// # Returns
/// - Current collateralization ratio in basis points
///
/// # Requirements
/// - Validates: Requirements 9.5
pub fn get_synthetic_collateralization_ratio(env: &Env, asset_id: u64) -> Result<u32, TipJarError> {
    get_collateralization_ratio(env, asset_id)
}

/// Calculates the required collateral for a minting amount
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
/// - `amount`: Amount of synthetic tokens to mint
///
/// # Returns
/// - Required collateral amount
///
/// # Requirements
/// - Validates: Requirements 9.6
pub fn calculate_synthetic_required_collateral(
    env: &Env,
    asset_id: u64,
    amount: i128,
) -> Result<i128, TipJarError> {
    calculate_required_collateral(env, asset_id, amount)
}

/// Calculates the redemption value for a token amount
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
/// - `amount`: Amount of synthetic tokens
///
/// # Returns
/// - Redemption value in backing tokens
///
/// # Requirements
/// - Validates: Requirements 9.7
pub fn calculate_synthetic_redemption_value(
    env: &Env,
    asset_id: u64,
    amount: i128,
) -> Result<i128, TipJarError> {
    calculate_redemption_value(env, asset_id, amount)
}
