//! Liquidation logic for under-collateralised positions.
//!
//! A position becomes liquidatable when its health factor drops below 1.0
//! (i.e. the collateral value no longer covers the debt at the liquidation
//! threshold). Any caller can act as a liquidator and repay the debt in
//! exchange for the collateral plus a penalty bonus.

use soroban_sdk::{panic_with_error, token, Address, Env};

use crate::{DataKey, TipJarError};

use super::{
    positions::{self, get_position_or_panic, save_position, set_total_debt},
    ratios, LiquidationRecord, BPS_DENOM, HEALTH_FACTOR_PRECISION,
};

// ── Public API ───────────────────────────────────────────────────────────────

/// Liquidates an under-collateralised position.
///
/// The liquidator repays `repay_amount` of the borrower's debt and receives
/// the equivalent collateral value plus a liquidation penalty bonus.
///
/// Steps:
/// 1. Verify the position's health factor is below 1.0.
/// 2. Transfer `repay_amount` from the liquidator into the contract.
/// 3. Calculate collateral to seize: `repay_amount * (BPS_DENOM + penalty_bps) / BPS_DENOM`.
/// 4. Reduce position debt and collateral accordingly.
/// 5. Transfer seized collateral to the liquidator.
/// 6. Record the liquidation event.
///
/// Panics if:
/// - Position does not exist.
/// - Position is already liquidated.
/// - Health factor is ≥ 1.0 (position is healthy).
/// - `repay_amount` exceeds the outstanding debt.
/// - Seized collateral would exceed available collateral.
pub fn liquidate(
    env: &Env,
    liquidator: &Address,
    depositor: &Address,
    collateral_token: &Address,
    repay_amount: i128,
) {
    liquidator.require_auth();

    if repay_amount <= 0 {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    let mut position = get_position_or_panic(env, depositor, collateral_token);

    if position.liquidated {
        panic_with_error!(env, TipJarError::CollateralPositionLiquidated);
    }

    // Check health factor — must be below 1.0 (below HEALTH_FACTOR_PRECISION)
    let hf = positions::health_factor(env, depositor, collateral_token);
    if hf >= HEALTH_FACTOR_PRECISION {
        panic_with_error!(env, TipJarError::CollateralPositionHealthy);
    }

    if repay_amount > position.debt_amount {
        panic_with_error!(env, TipJarError::RepayExceedsDebt);
    }

    let ratio = ratios::get_collateral_ratio(env, collateral_token);

    // Collateral seized = repay_amount * (BPS_DENOM + penalty_bps) / BPS_DENOM
    let collateral_seized = repay_amount
        * (BPS_DENOM as i128 + ratio.liquidation_penalty_bps as i128)
        / BPS_DENOM as i128;

    if collateral_seized > position.collateral_amount {
        panic_with_error!(env, TipJarError::InsufficientCollateral);
    }

    let penalty_amount = collateral_seized - repay_amount;

    // Transfer repayment from liquidator into the contract
    let token_client = token::Client::new(env, collateral_token);
    token_client.transfer(liquidator, &env.current_contract_address(), &repay_amount);

    // Update position
    position.debt_amount -= repay_amount;
    position.collateral_amount -= collateral_seized;
    position.updated_at = env.ledger().timestamp();

    // Mark fully liquidated if debt is cleared or collateral is exhausted
    if position.debt_amount == 0 || position.collateral_amount == 0 {
        position.liquidated = true;
    }

    save_position(env, &position);

    // Update global totals
    let total_collateral = positions::get_total_collateral(env, collateral_token);
    env.storage().persistent().set(
        &DataKey::TotalCollateral(collateral_token.clone()),
        &(total_collateral - collateral_seized),
    );

    let total_debt = positions::get_total_debt(env, collateral_token);
    set_total_debt(env, collateral_token, total_debt - repay_amount);

    // Transfer seized collateral to the liquidator
    token_client.transfer(&env.current_contract_address(), liquidator, &collateral_seized);

    // Record the liquidation
    let record_id = next_liquidation_id(env);
    let record = LiquidationRecord {
        id: record_id,
        depositor: depositor.clone(),
        liquidator: liquidator.clone(),
        token: collateral_token.clone(),
        collateral_seized,
        debt_repaid: repay_amount,
        penalty_amount,
        timestamp: env.ledger().timestamp(),
    };
    env.storage()
        .persistent()
        .set(&DataKey::LiquidationRecord(record_id), &record);

    env.events().publish(
        (soroban_sdk::symbol_short!("col_liq"),),
        (
            depositor.clone(),
            liquidator.clone(),
            collateral_token.clone(),
            collateral_seized,
            repay_amount,
            penalty_amount,
        ),
    );
}

/// Returns the liquidation record for `liquidation_id`, or `None`.
pub fn get_liquidation_record(env: &Env, liquidation_id: u64) -> Option<LiquidationRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::LiquidationRecord(liquidation_id))
}

/// Returns the total number of liquidations recorded.
pub fn liquidation_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::LiquidationCounter)
        .unwrap_or(0)
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn next_liquidation_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::LiquidationCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .persistent()
        .set(&DataKey::LiquidationCounter, &next);
    next
}
