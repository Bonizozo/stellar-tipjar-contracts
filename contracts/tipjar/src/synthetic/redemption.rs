//! Redemption engine for synthetic assets
//!
//! Manages the burning of synthetic tokens and distribution of underlying
//! value back to token holders.

use super::events::emit_synthetic_tokens_redeemed;
use super::oracle::get_oracle_price;
use super::supply::{update_collateral, update_supply};
use super::types::SyntheticAsset;
use crate::{CoreError, CreditError, DataKey, SyntheticKey, TipJarError};
use soroban_sdk::{token, Address, Env};

/// Calculates redemption value for a token amount
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
/// - `amount`: Amount of synthetic tokens
///
/// # Returns
/// - Redemption value in backing tokens
///
/// # Formula
/// - redemption_value = amount * oracle_price
///
/// # Requirements
/// - Validates: Requirements 5.2, 9.7
pub fn calculate_redemption_value(
    env: &Env,
    asset_id: u64,
    amount: i128,
) -> Result<i128, TipJarError> {
    // Validate amount is positive
    if amount <= 0 {
        return Err(CoreError::InvalidAmount.into());
    }

    // Get current oracle price
    let oracle_price = get_oracle_price(env, asset_id)?;

    // Calculate redemption value: amount * oracle_price
    let redemption_value = amount
        .checked_mul(oracle_price)
        .ok_or(CoreError::InvalidAmount)?;

    Ok(redemption_value)
}

/// Redeems synthetic tokens for underlying value
///
/// # Parameters
/// - `env`: Soroban environment
/// - `holder`: Address redeeming tokens
/// - `asset_id`: Identifier of the synthetic asset
/// - `amount`: Amount of synthetic tokens to redeem
///
/// # Returns
/// - Redemption value transferred to holder
///
/// # Requirements
/// - Validates: Requirements 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8, 5.9, 5.10, 12.3, 12.4, 12.6, 12.7
pub fn redeem(
    env: &Env,
    holder: &Address,
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

    // Verify holder owns sufficient synthetic tokens
    let balance_key = DataKey::Synthetic(SyntheticKey::SyntheticBalance(holder.clone(), asset_id));
    let current_balance: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0);

    if current_balance < amount {
        return Err(CoreError::InsufficientBalance.into());
    }

    // Calculate redemption value
    let redemption_value = calculate_redemption_value(env, asset_id, amount)?;

    // Check if tip pool has sufficient balance for redemption
    let creator_balance_key =
        DataKey::CreatorBalance(asset.creator.clone(), asset.backing_token.clone());
    let tip_pool_balance: i128 = env
        .storage()
        .persistent()
        .get(&creator_balance_key)
        .unwrap_or(0);

    if tip_pool_balance < redemption_value {
        return Err(CreditError::InsufficientPoolBalance.into());
    }

    // Burn synthetic tokens from holder (update SyntheticBalance storage)
    let new_balance = current_balance
        .checked_sub(amount)
        .ok_or(CoreError::InvalidAmount)?;

    if new_balance == 0 {
        env.storage().persistent().remove(&balance_key);
    } else {
        env.storage().persistent().set(&balance_key, &new_balance);
    }

    // Update total supply (decrease)
    update_supply(env, asset_id, -amount)?;

    // Unlock collateral from tip pool (update SyntheticCollateral storage)
    let collateral_key = DataKey::Synthetic(SyntheticKey::SyntheticCollateral(
        asset.creator.clone(),
        asset.backing_token.clone(),
    ));
    let current_locked: i128 = env.storage().persistent().get(&collateral_key).unwrap_or(0);
    let new_locked = current_locked
        .checked_sub(redemption_value)
        .ok_or(CoreError::InvalidAmount)?;

    if new_locked == 0 {
        env.storage().persistent().remove(&collateral_key);
    } else {
        env.storage().persistent().set(&collateral_key, &new_locked);
    }

    // Transfer redemption value from tip pool to holder
    let token_contract = token::Client::new(env, &asset.backing_token);
    token_contract.transfer(&asset.creator, holder, &redemption_value);

    // Update creator's tip pool balance
    let new_tip_pool_balance = tip_pool_balance
        .checked_sub(redemption_value)
        .ok_or(CoreError::InvalidAmount)?;

    if new_tip_pool_balance == 0 {
        env.storage().persistent().remove(&creator_balance_key);
    } else {
        env.storage()
            .persistent()
            .set(&creator_balance_key, &new_tip_pool_balance);
    }

    // Update total collateral (decrease)
    update_collateral(env, asset_id, -redemption_value)?;

    // Emit SyntheticTokensRedeemed event
    emit_synthetic_tokens_redeemed(env, asset_id, holder.clone(), amount, redemption_value);

    Ok(redemption_value)
}
