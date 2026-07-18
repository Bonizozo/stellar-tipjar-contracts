//! Type definitions for synthetic assets

use soroban_sdk::{contracttype, Address};

/// Represents a synthetic asset backed by a creator's tip pool
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntheticAsset {
    /// Unique identifier for this synthetic asset
    pub asset_id: u64,

    /// Creator whose tip pool backs this asset
    pub creator: Address,

    /// Token address of the backing collateral
    pub backing_token: Address,

    /// Total supply of synthetic tokens minted
    pub total_supply: i128,

    /// Collateralization ratio in basis points (10000 = 100%)
    /// Valid range: 10000-50000 (100%-500%)
    pub collateralization_ratio: u32,

    /// Timestamp when the asset was created
    pub created_at: u64,

    /// Current oracle price (backing_token per synthetic_token)
    pub oracle_price: i128,

    /// Total collateral locked in tip pool
    pub total_collateral: i128,

    /// Whether the asset is active (true) or paused (false)
    pub active: bool,
}
