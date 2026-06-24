//! Collateral position management.
//!
//! Handles locking collateral, borrowing against it, repaying debt, and
//! releasing collateral once debt is cleared.

use soroban_sdk::{panic_with_error, token, Address, Env, Vec};

use crate::{DataKey, TipJarError};

use super::{ratios, CollateralPosition, BPS_DENOM, HEALTH_FACTOR_PRECISION};

// ── Public API ───────────────────────────────────────────────────────────────

/// Locks `amount` of `collateral_token` from the caller's wallet as collateral,
/// creating or adding to an existing position.
///
/// The tokens are transferred from the depositor's wallet into the contract.
/// While locked, they cannot be withdrawn until released or liquidated.
///
/// Panics if:
/// - `collateral_token` is not enabled as collateral.
/// - `amount` is not positive.
pub fn lock_collateral(env: &Env, depositor: &Address, collateral_token: &Address, amount: i128) {
    depositor.require_auth();

    if amount <= 0 {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    if !ratios::is_collateral_enabled(env, collateral_token) {
        panic_with_error!(env, TipJarError::CollateralTokenNotEnabled);
    }

    // Transfer tokens from depositor into the contract
    let token_client = token::Client::new(env, collateral_token);
    token_client.transfer(depositor, &env.current_contract_address(), &amount);

    // Load or create position
    let mut position =
        get_position(env, depositor, collateral_token).unwrap_or(CollateralPosition {
            depositor: depositor.clone(),
            collateral_token: collateral_token.clone(),
            collateral_amount: 0,
            debt_amount: 0,
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
            liquidated: false,
        });

    if position.liquidated {
        panic_with_error!(env, TipJarError::CollateralPositionLiquidated);
    }

    position.collateral_amount += amount;
    position.updated_at = env.ledger().timestamp();

    save_position(env, &position);
    track_depositor_token(env, depositor, collateral_token);

    // Update global totals
    let total = get_total_collateral(env, collateral_token);
    set_total_collateral(env, collateral_token, total + amount);

    env.events().publish(
        (soroban_sdk::symbol_short!("col_lock"),),
        (
            depositor.clone(),
            collateral_token.clone(),
            amount,
            position.collateral_amount,
        ),
    );
}

/// Borrows `borrow_amount` against an existing collateral position.
///
/// The borrowed tokens are transferred from the contract to the borrower.
/// The debt is recorded in the position.
///
/// Panics if:
/// - No position exists.
/// - The borrow would push the position below the required collateral ratio.
/// - The position is already liquidated.
pub fn borrow_against_collateral(
    env: &Env,
    depositor: &Address,
    collateral_token: &Address,
    borrow_amount: i128,
) {
    depositor.require_auth();

    if borrow_amount <= 0 {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    let mut position = get_position_or_panic(env, depositor, collateral_token);

    if position.liquidated {
        panic_with_error!(env, TipJarError::CollateralPositionLiquidated);
    }

    let new_debt = position.debt_amount + borrow_amount;
    let ratio = ratios::get_collateral_ratio(env, collateral_token);

    // Ensure collateral_amount * BPS_DENOM >= new_debt * ratio_bps
    // i.e. collateral / debt >= ratio_bps / BPS_DENOM
    if position.collateral_amount * BPS_DENOM as i128 < new_debt * ratio.ratio_bps as i128 {
        panic_with_error!(env, TipJarError::InsufficientCollateral);
    }

    position.debt_amount = new_debt;
    position.updated_at = env.ledger().timestamp();
    save_position(env, &position);

    // Update global debt total
    let total_debt = get_total_debt(env, collateral_token);
    set_total_debt(env, collateral_token, total_debt + borrow_amount);

    // Transfer borrowed tokens to the depositor
    let token_client = token::Client::new(env, collateral_token);
    token_client.transfer(&env.current_contract_address(), depositor, &borrow_amount);

    env.events().publish(
        (soroban_sdk::symbol_short!("col_borr"),),
        (
            depositor.clone(),
            collateral_token.clone(),
            borrow_amount,
            new_debt,
        ),
    );
}

/// Repays `repay_amount` of debt for a collateral position.
///
/// Tokens are transferred from the repayer back into the contract.
/// If the full debt is repaid the position's debt is zeroed.
///
/// Panics if:
/// - No position exists.
/// - `repay_amount` exceeds the outstanding debt.
pub fn repay_debt(
    env: &Env,
    repayer: &Address,
    depositor: &Address,
    collateral_token: &Address,
    repay_amount: i128,
) {
    repayer.require_auth();

    if repay_amount <= 0 {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    let mut position = get_position_or_panic(env, depositor, collateral_token);

    if repay_amount > position.debt_amount {
        panic_with_error!(env, TipJarError::RepayExceedsDebt);
    }

    // Transfer repayment from repayer into the contract
    let token_client = token::Client::new(env, collateral_token);
    token_client.transfer(repayer, &env.current_contract_address(), &repay_amount);

    position.debt_amount -= repay_amount;
    position.updated_at = env.ledger().timestamp();
    save_position(env, &position);

    // Update global debt total
    let total_debt = get_total_debt(env, collateral_token);
    set_total_debt(env, collateral_token, total_debt - repay_amount);

    env.events().publish(
        (soroban_sdk::symbol_short!("col_repay"),),
        (
            depositor.clone(),
            collateral_token.clone(),
            repay_amount,
            position.debt_amount,
        ),
    );
}

/// Releases `release_amount` of collateral back to the depositor.
///
/// Only allowed when the remaining collateral (after release) still satisfies
/// the required collateral ratio against the outstanding debt.
/// If debt is zero, any amount up to the full collateral can be released.
///
/// Panics if:
/// - No position exists.
/// - Release would violate the collateral ratio.
/// - The position is liquidated.
pub fn release_collateral(
    env: &Env,
    depositor: &Address,
    collateral_token: &Address,
    release_amount: i128,
) {
    depositor.require_auth();

    if release_amount <= 0 {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    let mut position = get_position_or_panic(env, depositor, collateral_token);

    if position.liquidated {
        panic_with_error!(env, TipJarError::CollateralPositionLiquidated);
    }

    if release_amount > position.collateral_amount {
        panic_with_error!(env, TipJarError::InsufficientCollateral);
    }

    let remaining_collateral = position.collateral_amount - release_amount;

    // If there is outstanding debt, ensure the remaining collateral still
    // satisfies the required ratio
    if position.debt_amount > 0 {
        let ratio = ratios::get_collateral_ratio(env, collateral_token);
        if remaining_collateral * BPS_DENOM as i128
            < position.debt_amount * ratio.ratio_bps as i128
        {
            panic_with_error!(env, TipJarError::InsufficientCollateral);
        }
    }

    position.collateral_amount = remaining_collateral;
    position.updated_at = env.ledger().timestamp();
    save_position(env, &position);

    // Update global collateral total
    let total = get_total_collateral(env, collateral_token);
    set_total_collateral(env, collateral_token, total - release_amount);

    // Transfer released collateral back to the depositor
    let token_client = token::Client::new(env, collateral_token);
    token_client.transfer(&env.current_contract_address(), depositor, &release_amount);

    env.events().publish(
        (soroban_sdk::symbol_short!("col_rels"),),
        (
            depositor.clone(),
            collateral_token.clone(),
            release_amount,
            remaining_collateral,
        ),
    );
}

/// Returns the health factor of a position scaled by [`HEALTH_FACTOR_PRECISION`].
///
/// `health_factor = (collateral_amount * liquidation_threshold_bps) / (debt_amount * BPS_DENOM)`
///
/// A health factor ≥ `HEALTH_FACTOR_PRECISION` (1.0) means the position is
/// healthy. Below 1.0 it is eligible for liquidation.
///
/// Returns `i128::MAX` when there is no debt (perfectly healthy).
pub fn health_factor(env: &Env, depositor: &Address, collateral_token: &Address) -> i128 {
    let position = match get_position(env, depositor, collateral_token) {
        Some(p) => p,
        None => return i128::MAX,
    };

    if position.debt_amount == 0 {
        return i128::MAX;
    }

    let ratio = ratios::get_collateral_ratio(env, collateral_token);

    // health = (collateral * threshold_bps * PRECISION) / (debt * BPS_DENOM)
    position.collateral_amount
        * ratio.liquidation_threshold_bps as i128
        * HEALTH_FACTOR_PRECISION
        / (position.debt_amount * BPS_DENOM as i128)
}

/// Returns the collateral position for `(depositor, collateral_token)`, or
/// `None` if no position exists.
pub fn get_position(
    env: &Env,
    depositor: &Address,
    collateral_token: &Address,
) -> Option<CollateralPosition> {
    env.storage()
        .persistent()
        .get(&DataKey::CollateralPosition(
            depositor.clone(),
            collateral_token.clone(),
        ))
}

/// Returns the collateral position or panics with
/// [`TipJarError::CollateralPositionNotFound`].
pub fn get_position_or_panic(
    env: &Env,
    depositor: &Address,
    collateral_token: &Address,
) -> CollateralPosition {
    get_position(env, depositor, collateral_token)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::CollateralPositionNotFound))
}

/// Returns all token addresses for which `depositor` has active positions.
pub fn get_depositor_tokens(env: &Env, depositor: &Address) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::CollateralDepositorTokens(depositor.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Returns the total collateral locked for `token` across all positions.
pub fn get_total_collateral(env: &Env, token: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::TotalCollateral(token.clone()))
        .unwrap_or(0)
}

/// Returns the total outstanding debt for `token` across all positions.
pub fn get_total_debt(env: &Env, token: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::TotalDebt(token.clone()))
        .unwrap_or(0)
}

// ── Internal helpers ─────────────────────────────────────────────────────────

pub(super) fn save_position(env: &Env, position: &CollateralPosition) {
    env.storage().persistent().set(
        &DataKey::CollateralPosition(
            position.depositor.clone(),
            position.collateral_token.clone(),
        ),
        position,
    );
}

fn track_depositor_token(env: &Env, depositor: &Address, token: &Address) {
    let mut tokens: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::CollateralDepositorTokens(depositor.clone()))
        .unwrap_or_else(|| Vec::new(env));
    if !tokens.contains(token) {
        tokens.push_back(token.clone());
        env.storage().persistent().set(
            &DataKey::CollateralDepositorTokens(depositor.clone()),
            &tokens,
        );
    }
}

fn set_total_collateral(env: &Env, token: &Address, amount: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::TotalCollateral(token.clone()), &amount);
}

pub(super) fn set_total_debt(env: &Env, token: &Address, amount: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::TotalDebt(token.clone()), &amount);
}
