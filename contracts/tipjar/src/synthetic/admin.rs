//! Administration functions for synthetic assets
//!
//! Provides creator controls for managing synthetic assets including
//! pause, resume, and parameter adjustments.

use super::events::{
    emit_collateralization_updated, emit_synthetic_asset_created, emit_synthetic_asset_paused,
    emit_synthetic_asset_resumed,
};
use super::supply::{get_collateralization_ratio, update_collateral};
use super::types::SyntheticAsset;
use crate::{CoreError, CreditError, DataKey, SyntheticKey, TipJarError};
use soroban_sdk::{token, Address, Env, Vec};

/// Creates a new synthetic asset
///
/// # Parameters
/// - `env`: Soroban environment
/// - `creator`: Creator address
/// - `backing_token`: Token address for collateral
/// - `collateralization_ratio`: Ratio in basis points (10000-50000)
///
/// # Returns
/// - New asset identifier
///
/// # Requirements
/// - Validates: Requirements 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 11.9
pub fn create_synthetic_asset(
    env: &Env,
    creator: &Address,
    backing_token: &Address,
    collateralization_ratio: u32,
) -> Result<u64, TipJarError> {
    // Verify collateralization ratio is between 10000 and 50000 bps (100%-500%)
    if !(10000..=50000).contains(&collateralization_ratio) {
        return Err(CreditError::InvalidCollateralizationRatio.into());
    }

    // Verify backing token exists in creator's tip pool
    let creator_balance_key = DataKey::CreatorBalance(creator.clone(), backing_token.clone());
    let tip_pool_balance: i128 = env
        .storage()
        .persistent()
        .get(&creator_balance_key)
        .unwrap_or(0);

    if tip_pool_balance == 0 {
        return Err(CreditError::TokenNotInPool.into());
    }

    // Verify creator has sufficient tip pool balance (at least some minimum)
    // For now, we just check that balance > 0, but in practice might want a minimum threshold
    if tip_pool_balance <= 0 {
        return Err(CreditError::InsufficientCollateral.into());
    }

    // Generate unique asset_id using SyntheticAssetCounter
    let counter_key = DataKey::Synthetic(SyntheticKey::SyntheticAssetCounter);
    let current_counter: u64 = env.storage().instance().get(&counter_key).unwrap_or(0);
    let asset_id = current_counter + 1;
    env.storage().instance().set(&counter_key, &asset_id);

    // Create SyntheticAsset record with initial values
    let current_timestamp = env.ledger().timestamp();
    let asset = SyntheticAsset {
        asset_id,
        creator: creator.clone(),
        backing_token: backing_token.clone(),
        total_supply: 0,
        collateralization_ratio,
        created_at: current_timestamp,
        oracle_price: 10_000_000, // Initial price: 1 unit of backing token (10^7 stroops)
        total_collateral: 0,
        active: true,
    };

    // Store asset record in persistent storage
    let asset_key = DataKey::Synthetic(SyntheticKey::SyntheticAsset(asset_id));
    env.storage().persistent().set(&asset_key, &asset);

    // Add asset_id to creator's asset list
    let creator_assets_key =
        DataKey::Synthetic(SyntheticKey::CreatorSyntheticAssets(creator.clone()));
    let mut creator_assets: Vec<u64> = env
        .storage()
        .persistent()
        .get(&creator_assets_key)
        .unwrap_or(Vec::new(env));
    creator_assets.push_back(asset_id);
    env.storage()
        .persistent()
        .set(&creator_assets_key, &creator_assets);

    // Emit SyntheticAssetCreated event
    emit_synthetic_asset_created(
        env,
        asset_id,
        creator.clone(),
        backing_token.clone(),
        collateralization_ratio,
    );

    Ok(asset_id)
}

/// Pauses a synthetic asset (prevents new minting)
///
/// # Parameters
/// - `env`: Soroban environment
/// - `creator`: Creator address (must match asset creator)
/// - `asset_id`: Identifier of the synthetic asset
///
/// # Requirements
/// - Validates: Requirements 8.1, 8.2, 8.8, 8.10, 12.1, 12.9
pub fn pause_synthetic_asset(
    env: &Env,
    creator: &Address,
    asset_id: u64,
) -> Result<(), TipJarError> {
    // Retrieve the synthetic asset
    let asset_key = DataKey::Synthetic(SyntheticKey::SyntheticAsset(asset_id));
    let mut asset: SyntheticAsset = env
        .storage()
        .persistent()
        .get(&asset_key)
        .ok_or(CreditError::SyntheticAssetNotFound)?;

    // Verify caller is asset creator
    if asset.creator != *creator {
        return Err(CoreError::Unauthorized.into());
    }

    // Set active status to false
    asset.active = false;
    env.storage().persistent().set(&asset_key, &asset);

    // Emit SyntheticAssetPaused event
    emit_synthetic_asset_paused(env, asset_id);

    Ok(())
}

/// Resumes a paused synthetic asset
///
/// # Parameters
/// - `env`: Soroban environment
/// - `creator`: Creator address (must match asset creator)
/// - `asset_id`: Identifier of the synthetic asset
///
/// # Requirements
/// - Validates: Requirements 8.4, 8.5, 8.9, 8.10, 12.1, 12.9
pub fn resume_synthetic_asset(
    env: &Env,
    creator: &Address,
    asset_id: u64,
) -> Result<(), TipJarError> {
    // Retrieve the synthetic asset
    let asset_key = DataKey::Synthetic(SyntheticKey::SyntheticAsset(asset_id));
    let mut asset: SyntheticAsset = env
        .storage()
        .persistent()
        .get(&asset_key)
        .ok_or(CreditError::SyntheticAssetNotFound)?;

    // Verify caller is asset creator
    if asset.creator != *creator {
        return Err(CoreError::Unauthorized.into());
    }

    // Verify collateralization requirements are met
    let current_ratio = get_collateralization_ratio(env, asset_id)?;
    if current_ratio < asset.collateralization_ratio {
        return Err(CreditError::CollateralizationViolation.into());
    }

    // Set active status to true
    asset.active = true;
    env.storage().persistent().set(&asset_key, &asset);

    // Emit SyntheticAssetResumed event
    emit_synthetic_asset_resumed(env, asset_id);

    Ok(())
}

/// Updates collateralization ratio for future minting
///
/// # Parameters
/// - `env`: Soroban environment
/// - `creator`: Creator address (must match asset creator)
/// - `asset_id`: Identifier of the synthetic asset
/// - `new_ratio`: New ratio in basis points (10000-50000)
///
/// # Requirements
/// - Validates: Requirements 8.6, 8.7, 8.10, 12.1, 12.9
pub fn update_collateralization_ratio(
    env: &Env,
    creator: &Address,
    asset_id: u64,
    new_ratio: u32,
) -> Result<(), TipJarError> {
    // Verify new ratio is between 10000 and 50000 bps
    if !(10000..=50000).contains(&new_ratio) {
        return Err(CreditError::InvalidCollateralizationRatio.into());
    }

    // Retrieve the synthetic asset
    let asset_key = DataKey::Synthetic(SyntheticKey::SyntheticAsset(asset_id));
    let mut asset: SyntheticAsset = env
        .storage()
        .persistent()
        .get(&asset_key)
        .ok_or(CreditError::SyntheticAssetNotFound)?;

    // Verify caller is asset creator
    if asset.creator != *creator {
        return Err(CoreError::Unauthorized.into());
    }

    // Update collateralization_ratio field
    asset.collateralization_ratio = new_ratio;
    env.storage().persistent().set(&asset_key, &asset);

    // Emit CollateralizationUpdated event
    emit_collateralization_updated(env, asset_id, new_ratio);

    Ok(())
}

/// Adds collateral to improve collateralization ratio
///
/// # Parameters
/// - `env`: Soroban environment
/// - `creator`: Creator address
/// - `asset_id`: Identifier of the synthetic asset
/// - `amount`: Amount of collateral to add
///
/// # Requirements
/// - Validates: Requirements 7.5, 7.6, 12.1
pub fn add_collateral(
    env: &Env,
    creator: &Address,
    asset_id: u64,
    amount: i128,
) -> Result<(), TipJarError> {
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

    // Verify caller is asset creator
    if asset.creator != *creator {
        return Err(CoreError::Unauthorized.into());
    }

    // Transfer collateral from creator to tip pool (this is essentially adding to their own pool)
    // In practice, this might involve transferring from creator's wallet to the contract
    let token_contract = token::Client::new(env, &asset.backing_token);
    token_contract.transfer(creator, creator, &amount); // Self-transfer to validate balance

    // Update creator's tip pool balance
    let creator_balance_key = DataKey::CreatorBalance(creator.clone(), asset.backing_token.clone());
    let current_balance: i128 = env
        .storage()
        .persistent()
        .get(&creator_balance_key)
        .unwrap_or(0);
    let new_balance = current_balance
        .checked_add(amount)
        .ok_or(CoreError::InvalidAmount)?;
    env.storage()
        .persistent()
        .set(&creator_balance_key, &new_balance);

    // Update total collateral via update_collateral()
    update_collateral(env, asset_id, amount)?;

    Ok(())
}
