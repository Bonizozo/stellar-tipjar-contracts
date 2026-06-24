//! Event definitions for synthetic assets
//!
//! Defines all events emitted by synthetic asset operations for
//! off-chain tracking and external system integration.

use soroban_sdk::{contracttype, symbol_short, Address, Env};

/// Emitted when a synthetic asset is created
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntheticAssetCreatedEvent {
    pub asset_id: u64,
    pub creator: Address,
    pub backing_token: Address,
    pub collateralization_ratio: u32,
    pub timestamp: u64,
}

/// Emitted when synthetic tokens are minted
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntheticTokensMintedEvent {
    pub asset_id: u64,
    pub minter: Address,
    pub amount: i128,
    pub collateral_provided: i128,
    pub timestamp: u64,
}

/// Emitted when synthetic tokens are redeemed
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntheticTokensRedeemedEvent {
    pub asset_id: u64,
    pub redeemer: Address,
    pub amount: i128,
    pub value_received: i128,
    pub timestamp: u64,
}

/// Emitted when oracle price is updated
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceUpdatedEvent {
    pub asset_id: u64,
    pub new_price: i128,
    pub timestamp: u64,
}

/// Emitted when total supply changes
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupplyUpdatedEvent {
    pub asset_id: u64,
    pub new_total_supply: i128,
    pub timestamp: u64,
}

/// Emitted when total collateral changes
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollateralUpdatedEvent {
    pub asset_id: u64,
    pub new_total_collateral: i128,
    pub timestamp: u64,
}

/// Emitted when a synthetic asset is paused
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntheticAssetPausedEvent {
    pub asset_id: u64,
    pub timestamp: u64,
}

/// Emitted when a synthetic asset is resumed
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntheticAssetResumedEvent {
    pub asset_id: u64,
    pub timestamp: u64,
}

/// Emitted when collateralization ratio is updated
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollateralizationUpdatedEvent {
    pub asset_id: u64,
    pub new_ratio: u32,
    pub timestamp: u64,
}

// Event emission helper functions

/// Emits a SyntheticAssetCreated event
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Unique identifier for the synthetic asset
/// - `creator`: Creator address
/// - `backing_token`: Token address for collateral
/// - `collateralization_ratio`: Ratio in basis points
pub fn emit_synthetic_asset_created(
    env: &Env,
    asset_id: u64,
    creator: Address,
    backing_token: Address,
    collateralization_ratio: u32,
) {
    let timestamp = env.ledger().timestamp();
    let event = SyntheticAssetCreatedEvent {
        asset_id,
        creator,
        backing_token,
        collateralization_ratio,
        timestamp,
    };
    env.events().publish((symbol_short!("syn_crt"),), event);
}

/// Emits a SyntheticTokensMinted event
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
/// - `minter`: Address minting tokens
/// - `amount`: Amount of synthetic tokens minted
/// - `collateral_provided`: Amount of collateral provided
pub fn emit_synthetic_tokens_minted(
    env: &Env,
    asset_id: u64,
    minter: Address,
    amount: i128,
    collateral_provided: i128,
) {
    let timestamp = env.ledger().timestamp();
    let event = SyntheticTokensMintedEvent {
        asset_id,
        minter,
        amount,
        collateral_provided,
        timestamp,
    };
    env.events().publish((symbol_short!("syn_mnt"),), event);
}

/// Emits a SyntheticTokensRedeemed event
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
/// - `redeemer`: Address redeeming tokens
/// - `amount`: Amount of synthetic tokens redeemed
/// - `value_received`: Value received in backing tokens
pub fn emit_synthetic_tokens_redeemed(
    env: &Env,
    asset_id: u64,
    redeemer: Address,
    amount: i128,
    value_received: i128,
) {
    let timestamp = env.ledger().timestamp();
    let event = SyntheticTokensRedeemedEvent {
        asset_id,
        redeemer,
        amount,
        value_received,
        timestamp,
    };
    env.events().publish((symbol_short!("syn_rdm"),), event);
}

/// Emits a PriceUpdated event
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
/// - `new_price`: Updated oracle price
pub fn emit_price_updated(env: &Env, asset_id: u64, new_price: i128) {
    let timestamp = env.ledger().timestamp();
    let event = PriceUpdatedEvent {
        asset_id,
        new_price,
        timestamp,
    };
    env.events().publish((symbol_short!("syn_prc"),), event);
}

/// Emits a SupplyUpdated event
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
/// - `new_total_supply`: Updated total supply
pub fn emit_supply_updated(env: &Env, asset_id: u64, new_total_supply: i128) {
    let timestamp = env.ledger().timestamp();
    let event = SupplyUpdatedEvent {
        asset_id,
        new_total_supply,
        timestamp,
    };
    env.events().publish((symbol_short!("syn_sup"),), event);
}

/// Emits a CollateralUpdated event
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
/// - `new_total_collateral`: Updated total collateral
pub fn emit_collateral_updated(env: &Env, asset_id: u64, new_total_collateral: i128) {
    let timestamp = env.ledger().timestamp();
    let event = CollateralUpdatedEvent {
        asset_id,
        new_total_collateral,
        timestamp,
    };
    env.events().publish((symbol_short!("syn_col"),), event);
}

/// Emits a SyntheticAssetPaused event
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
pub fn emit_synthetic_asset_paused(env: &Env, asset_id: u64) {
    let timestamp = env.ledger().timestamp();
    let event = SyntheticAssetPausedEvent {
        asset_id,
        timestamp,
    };
    env.events().publish((symbol_short!("syn_pse"),), event);
}

/// Emits a SyntheticAssetResumed event
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
pub fn emit_synthetic_asset_resumed(env: &Env, asset_id: u64) {
    let timestamp = env.ledger().timestamp();
    let event = SyntheticAssetResumedEvent {
        asset_id,
        timestamp,
    };
    env.events().publish((symbol_short!("syn_rsm"),), event);
}

/// Emits a CollateralizationUpdated event
///
/// # Parameters
/// - `env`: Soroban environment
/// - `asset_id`: Identifier of the synthetic asset
/// - `new_ratio`: Updated collateralization ratio in basis points
pub fn emit_collateralization_updated(env: &Env, asset_id: u64, new_ratio: u32) {
    let timestamp = env.ledger().timestamp();
    let event = CollateralizationUpdatedEvent {
        asset_id,
        new_ratio,
        timestamp,
    };
    env.events().publish((symbol_short!("syn_rat"),), event);
}
