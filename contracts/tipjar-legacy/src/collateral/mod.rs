//! Collateralization mechanism for using tips as collateral in DeFi protocols.
//!
//! This module allows users to lock their tip balances as collateral to borrow
//! against them. It implements:
//!
//! - Collateral ratios per token (loan-to-value limits)
//! - Collateral locking (prevents withdrawal while collateralized)
//! - Liquidation logic when health factor drops below threshold
//! - Collateral release when debt is repaid
//! - Position tracking per depositor

pub mod liquidation;
pub mod positions;
pub mod ratios;

use soroban_sdk::{contracttype, Address};

// ── Basis-point constants ────────────────────────────────────────────────────

/// Basis-point denominator (10 000 = 100%).
pub const BPS_DENOM: u32 = 10_000;

/// Default collateral ratio: 150% (15 000 bps). A user must lock 1.5× the
/// value they wish to borrow.
pub const DEFAULT_COLLATERAL_RATIO_BPS: u32 = 15_000;

/// Minimum allowed collateral ratio (110%).
pub const MIN_COLLATERAL_RATIO_BPS: u32 = 11_000;

/// Maximum allowed collateral ratio (500%).
pub const MAX_COLLATERAL_RATIO_BPS: u32 = 50_000;

/// Liquidation threshold: position becomes liquidatable when collateral value
/// falls to 120% of debt value (12 000 bps).
pub const DEFAULT_LIQUIDATION_THRESHOLD_BPS: u32 = 12_000;

/// Liquidation penalty charged to the borrower (5%).
pub const DEFAULT_LIQUIDATION_PENALTY_BPS: u32 = 500;

/// Health factor precision multiplier (1 000 000 = 1.0).
pub const HEALTH_FACTOR_PRECISION: i128 = 1_000_000;

// ── Data types ───────────────────────────────────────────────────────────────

/// Collateral ratio configuration for a specific token.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollateralRatio {
    /// Token this ratio applies to.
    pub token: Address,
    /// Minimum collateral ratio in basis points (e.g. 15 000 = 150%).
    pub ratio_bps: u32,
    /// Liquidation threshold in basis points (e.g. 12 000 = 120%).
    pub liquidation_threshold_bps: u32,
    /// Liquidation penalty in basis points (e.g. 500 = 5%).
    pub liquidation_penalty_bps: u32,
    /// Whether this token is accepted as collateral.
    pub enabled: bool,
}

/// A collateral position held by a depositor for a specific token.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollateralPosition {
    /// Owner of the collateral.
    pub depositor: Address,
    /// Token used as collateral.
    pub collateral_token: Address,
    /// Amount of collateral locked.
    pub collateral_amount: i128,
    /// Outstanding debt (borrowed amount) denominated in the same token units.
    pub debt_amount: i128,
    /// Timestamp when the position was opened.
    pub created_at: u64,
    /// Timestamp of the last update (borrow, repay, or partial liquidation).
    pub updated_at: u64,
    /// Whether the position has been fully liquidated.
    pub liquidated: bool,
}

/// Summary of a liquidation event stored for audit purposes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidationRecord {
    /// Unique liquidation ID.
    pub id: u64,
    /// Address that was liquidated.
    pub depositor: Address,
    /// Liquidator who triggered the liquidation.
    pub liquidator: Address,
    /// Token involved.
    pub token: Address,
    /// Collateral amount seized.
    pub collateral_seized: i128,
    /// Debt amount repaid by the liquidator.
    pub debt_repaid: i128,
    /// Penalty amount charged.
    pub penalty_amount: i128,
    /// Timestamp of the liquidation.
    pub timestamp: u64,
}
