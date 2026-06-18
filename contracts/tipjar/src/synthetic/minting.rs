//! Minting engine for synthetic assets
//!
//! Handles the creation of new synthetic tokens by accepting collateral
//! and calculating appropriate token amounts based on oracle prices and
//! collateralization ratios.

use super::events::emit_synthetic_tokens_minted;
use super::oracle::get_oracle_price;
use super::supply::{update_collateral, update_supply};
use super::types::SyntheticAsset;
use crate::{CoreError, CreditError, DataKey, SyntheticKey, TipJarError};
use soroban_sdk::{token, Address, Env};

/// Calculates required collateral for minting synthetic tokens
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
/// - `amount`: Amount of synthetic tokens to mint
///
/// # Returns
/// - Required collateral amount
///
/// # Formula
/// - collateral = (amount * oracle_price * collateralization_ratio) / 10000
///
/// # Requirements
/// - Validates: Requirements 3.2, 9.6
pub fn calculate_required_collateral(
    env: &Env,
    asset_id: u64,
    amount: i128,
) -> Result<i128, TipJarError> {
    // Validate amount is positive
    if amount <= 0 {
        return Err(CoreError::InvalidAmount.into());
    }

    // Retrieve the synthetic asset
    let asset_key = DataKey::Synthetic(SyntheticKey::SyntheticAsset(asset_id));
    let asset: SyntheticAsset = env
        .storage()
        .persistent()
        .get(&asset_key)
        .ok_or(CreditError::SyntheticAssetNotFound)?;

    // Get current oracle price
    let oracle_price = get_oracle_price(env, asset_id)?;

    // Calculate required collateral: (amount * oracle_price * collateralization_ratio) / 10000
    let collateral = amount
        .checked_mul(oracle_price)
        .ok_or(CoreError::InvalidAmount)?
        .checked_mul(asset.collateralization_ratio as i128)
        .ok_or(CoreError::InvalidAmount)?
        .checked_div(10000)
        .ok_or(CoreError::InvalidAmount)?;

    Ok(collateral)
}

/// Mints synthetic tokens by providing collateral
///
/// # Parameters
/// - `env`: Soroban environment
/// - `user`: Address requesting to mint tokens
/// - `asset_id`: Identifier of the synthetic asset
/// - `amount`: Amount of synthetic tokens to mint
///
/// # Returns
/// - Amount of collateral required and transferred
///
/// # Requirements
/// - Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 3.10, 12.2, 12.4, 12.5, 12.7
pub fn mint(env: &Env, user: &Address, asset_id: u64, amount: i128) -> Result<i128, TipJarError> {
    // Validate amount is positive
    if amount <= 0 {
        return Err(CoreError::InvalidAmount.into());
    }

    // Verify synthetic asset exists and is active
    let asset_key = DataKey::Synthetic(SyntheticKey::SyntheticAsset(asset_id));
    let asset: SyntheticAsset = env
        .storage()
        .persistent()
        .get(&asset_key)
        .ok_or(CreditError::SyntheticAssetNotFound)?;

    if !asset.active {
        return Err(CreditError::SyntheticAssetInactive.into());
    }

    // Calculate required collateral
    let required_collateral = calculate_required_collateral(env, asset_id, amount)?;

    // Get backing token contract
    let token_contract = token::Client::new(env, &asset.backing_token);

    // Transfer collateral from user to tip pool (creator)
    // This validates that user has sufficient balance and authorization
    token_contract.transfer(user, &asset.creator, &required_collateral);

    // Lock collateral in tip pool (update SyntheticCollateral storage)
    let collateral_key = DataKey::Synthetic(SyntheticKey::SyntheticCollateral(
        asset.creator.clone(),
        asset.backing_token.clone(),
    ));
    let current_locked: i128 = env.storage().persistent().get(&collateral_key).unwrap_or(0);
    let new_locked = current_locked
        .checked_add(required_collateral)
        .ok_or(CoreError::InvalidAmount)?;
    env.storage().persistent().set(&collateral_key, &new_locked);

    // Mint synthetic tokens to user (update SyntheticBalance storage)
    let balance_key = DataKey::Synthetic(SyntheticKey::SyntheticBalance(user.clone(), asset_id));
    let current_balance: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0);
    let new_balance = current_balance
        .checked_add(amount)
        .ok_or(CoreError::InvalidAmount)?;
    env.storage().persistent().set(&balance_key, &new_balance);

    // Update total supply
    update_supply(env, asset_id, amount)?;

    // Update total collateral
    update_collateral(env, asset_id, required_collateral)?;

    // Emit SyntheticTokensMinted event
    emit_synthetic_tokens_minted(env, asset_id, user.clone(), amount, required_collateral);

    Ok(required_collateral)
}
