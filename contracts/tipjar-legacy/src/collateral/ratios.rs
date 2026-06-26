//! Collateral ratio management.
//!
//! Admins configure per-token collateral ratios, liquidation thresholds, and
//! liquidation penalties. These values govern how much a user can borrow
//! against their locked collateral.

use soroban_sdk::{panic_with_error, Address, Env};

use crate::{DataKey, TipJarError};

use super::{
    CollateralRatio, DEFAULT_COLLATERAL_RATIO_BPS, DEFAULT_LIQUIDATION_PENALTY_BPS,
    DEFAULT_LIQUIDATION_THRESHOLD_BPS, MAX_COLLATERAL_RATIO_BPS, MIN_COLLATERAL_RATIO_BPS,
};

// ── Public API ───────────────────────────────────────────────────────────────

/// Sets (or updates) the collateral ratio configuration for `token`.
///
/// - `ratio_bps`: minimum collateral ratio in basis points (e.g. 15 000 = 150%).
/// - `liquidation_threshold_bps`: threshold below which a position is liquidatable.
/// - `liquidation_penalty_bps`: penalty charged on liquidation.
///
/// Panics with [`TipJarError::Unauthorized`] if `admin` is not the stored admin.
/// Panics with [`TipJarError::InvalidCollateralRatio`] if ratio values are out of range.
pub fn set_collateral_ratio(
    env: &Env,
    admin: &Address,
    token: &Address,
    ratio_bps: u32,
    liquidation_threshold_bps: u32,
    liquidation_penalty_bps: u32,
) {
    admin.require_auth();
    require_admin(env, admin);
    validate_ratio_params(env, ratio_bps, liquidation_threshold_bps, liquidation_penalty_bps);

    let ratio = CollateralRatio {
        token: token.clone(),
        ratio_bps,
        liquidation_threshold_bps,
        liquidation_penalty_bps,
        enabled: true,
    };

    env.storage()
        .persistent()
        .set(&DataKey::CollateralRatio(token.clone()), &ratio);

    env.events().publish(
        (soroban_sdk::symbol_short!("col_ratio"),),
        (token.clone(), ratio_bps, liquidation_threshold_bps),
    );
}

/// Enables or disables a token as accepted collateral. Admin only.
pub fn set_collateral_enabled(env: &Env, admin: &Address, token: &Address, enabled: bool) {
    admin.require_auth();
    require_admin(env, admin);

    let mut ratio = get_ratio_or_default(env, token);
    ratio.enabled = enabled;
    env.storage()
        .persistent()
        .set(&DataKey::CollateralRatio(token.clone()), &ratio);

    env.events().publish(
        (soroban_sdk::symbol_short!("col_enbl"),),
        (token.clone(), enabled),
    );
}

/// Returns the collateral ratio config for `token`, or a default if not set.
pub fn get_collateral_ratio(env: &Env, token: &Address) -> CollateralRatio {
    get_ratio_or_default(env, token)
}

/// Returns `true` if `token` is accepted as collateral.
pub fn is_collateral_enabled(env: &Env, token: &Address) -> bool {
    get_ratio_or_default(env, token).enabled
}

/// Computes the maximum borrowable amount given `collateral_amount` and the
/// ratio config for `token`.
///
/// `max_borrow = collateral_amount * BPS_DENOM / ratio_bps`
pub fn max_borrowable(env: &Env, token: &Address, collateral_amount: i128) -> i128 {
    if collateral_amount <= 0 {
        return 0;
    }
    let ratio = get_ratio_or_default(env, token);
    collateral_amount * super::BPS_DENOM as i128 / ratio.ratio_bps as i128
}

// ── Internal helpers ─────────────────────────────────────────────────────────

pub(super) fn get_ratio_or_default(env: &Env, token: &Address) -> CollateralRatio {
    env.storage()
        .persistent()
        .get(&DataKey::CollateralRatio(token.clone()))
        .unwrap_or(CollateralRatio {
            token: token.clone(),
            ratio_bps: DEFAULT_COLLATERAL_RATIO_BPS,
            liquidation_threshold_bps: DEFAULT_LIQUIDATION_THRESHOLD_BPS,
            liquidation_penalty_bps: DEFAULT_LIQUIDATION_PENALTY_BPS,
            enabled: false,
        })
}

fn require_admin(env: &Env, admin: &Address) {
    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::Unauthorized));
    if *admin != stored_admin {
        panic_with_error!(env, TipJarError::Unauthorized);
    }
}

fn validate_ratio_params(
    env: &Env,
    ratio_bps: u32,
    liquidation_threshold_bps: u32,
    liquidation_penalty_bps: u32,
) {
    // Collateral ratio must be within allowed bounds
    if ratio_bps < MIN_COLLATERAL_RATIO_BPS || ratio_bps > MAX_COLLATERAL_RATIO_BPS {
        panic_with_error!(env, TipJarError::InvalidCollateralRatio);
    }
    // Liquidation threshold must be below the collateral ratio (otherwise
    // positions would be immediately liquidatable)
    if liquidation_threshold_bps >= ratio_bps {
        panic_with_error!(env, TipJarError::InvalidCollateralRatio);
    }
    // Liquidation threshold must be at least 100% (10 000 bps)
    if liquidation_threshold_bps < super::BPS_DENOM {
        panic_with_error!(env, TipJarError::InvalidCollateralRatio);
    }
    // Penalty must not exceed 50% (5 000 bps)
    if liquidation_penalty_bps > 5_000 {
        panic_with_error!(env, TipJarError::InvalidCollateralRatio);
    }
}
