//! Risk management for the derivatives platform.
//!
//! Responsibilities:
//! - Enforce per-account position limits.
//! - Check margin health (initial and maintenance).
//! - Compute portfolio-level health factor.
//! - Identify which side of a contract is under-margined.

use super::{
    DerivativeContract, DerivativeKind, DerivativeStatus, DerivativesConfig,
    get_active_ids, get_account_contracts, get_contract, get_portfolio,
    BPS, PRICE_PRECISION,
};
use soroban_sdk::{Address, Env};

/// Health factor precision: 1_000_000 = 1.0 (healthy threshold).
pub const HEALTH_PRECISION: i128 = 1_000_000;

/// Portfolio health summary.
pub struct PortfolioHealth {
    /// Total collateral locked (token base units).
    pub total_collateral: i128,
    /// Total notional exposure (token base units).
    pub total_notional: i128,
    /// Unrealised P&L (can be negative).
    pub unrealised_pnl: i128,
    /// Health factor: (collateral + unrealised_pnl) / required_maintenance_margin.
    /// Values < HEALTH_PRECISION indicate under-margined.
    pub health_factor: i128,
    /// Number of active contracts.
    pub active_count: u32,
}

// ── Position limit checks ────────────────────────────────────────────────────

/// Panic if the account has reached the maximum number of open positions.
pub fn check_position_limit(env: &Env, account: &Address, cfg: &DerivativesConfig) {
    let portfolio = get_portfolio(env, account);
    let total = portfolio.initiated_count + portfolio.matched_count;
    assert!(
        total < cfg.max_positions,
        "position limit reached"
    );
}

// ── Margin health ────────────────────────────────────────────────────────────

/// Check whether either side of a contract is below maintenance margin.
/// Returns `(party_a_under, party_b_under)`.
pub fn check_margin_health(
    dc: &DerivativeContract,
    cfg: &DerivativesConfig,
) -> (bool, bool) {
    let notional_value = dc.notional * dc.mark_price / PRICE_PRECISION;
    let maintenance = notional_value * cfg.maintenance_margin_bps / BPS;

    // Unrealised P&L for party_a (long for futures, writer for options)
    let pnl_a = unrealised_pnl_a(dc);
    let effective_a = dc.collateral_a + pnl_a;

    let under_a = effective_a < maintenance;

    let under_b = if dc.party_b.is_some() {
        let pnl_b = -pnl_a; // zero-sum
        let effective_b = dc.collateral_b + pnl_b;
        effective_b < maintenance
    } else {
        false
    };

    (under_a, under_b)
}

/// Compute the unrealised P&L for party_a given the current mark price.
/// - Futures/Swap: long P&L = (mark - strike) * notional / PRICE_PRECISION
/// - Call option: intrinsic = max(mark - strike, 0) * notional / PRICE_PRECISION
/// - Put option: intrinsic = max(strike - mark, 0) * notional / PRICE_PRECISION
pub fn unrealised_pnl_a(dc: &DerivativeContract) -> i128 {
    match dc.kind {
        DerivativeKind::Future | DerivativeKind::Swap => {
            (dc.mark_price - dc.strike) * dc.notional / PRICE_PRECISION
        }
        DerivativeKind::Call => {
            let intrinsic = (dc.mark_price - dc.strike).max(0);
            intrinsic * dc.notional / PRICE_PRECISION
        }
        DerivativeKind::Put => {
            let intrinsic = (dc.strike - dc.mark_price).max(0);
            intrinsic * dc.notional / PRICE_PRECISION
        }
    }
}

// ── Portfolio health ─────────────────────────────────────────────────────────

/// Compute the overall portfolio health for an account.
pub fn portfolio_health(env: &Env, account: &Address, cfg: &DerivativesConfig) -> PortfolioHealth {
    let ids = get_account_contracts(env, account);
    let mut total_collateral: i128 = 0;
    let mut total_notional: i128 = 0;
    let mut unrealised_pnl: i128 = 0;
    let mut active_count: u32 = 0;

    for i in 0..ids.len() {
        let id = ids.get(i).unwrap();
        if let Some(dc) = get_contract(env, id) {
            if dc.status != DerivativeStatus::Active {
                continue;
            }
            active_count += 1;
            let notional_value = dc.notional * dc.mark_price / PRICE_PRECISION;
            total_notional += notional_value;

            if dc.party_a == *account {
                total_collateral += dc.collateral_a;
                unrealised_pnl += unrealised_pnl_a(&dc);
            } else if dc.party_b.as_ref() == Some(account) {
                total_collateral += dc.collateral_b;
                unrealised_pnl -= unrealised_pnl_a(&dc); // party_b is opposite
            }
        }
    }

    let required_maintenance = total_notional * cfg.maintenance_margin_bps / BPS;
    let health_factor = if required_maintenance == 0 {
        HEALTH_PRECISION * 10 // no exposure → very healthy
    } else {
        (total_collateral + unrealised_pnl) * HEALTH_PRECISION / required_maintenance
    };

    PortfolioHealth {
        total_collateral,
        total_notional,
        unrealised_pnl,
        health_factor,
        active_count,
    }
}

/// Returns true if the account's portfolio is healthy (health_factor >= 1.0).
pub fn is_healthy(env: &Env, account: &Address, cfg: &DerivativesConfig) -> bool {
    portfolio_health(env, account, cfg).health_factor >= HEALTH_PRECISION
}

/// Scan all active contracts and return IDs of contracts with at least one
/// under-margined side. Useful for liquidation bots.
pub fn find_liquidatable(env: &Env, cfg: &DerivativesConfig) -> soroban_sdk::Vec<u64> {
    let active = get_active_ids(env);
    let mut result: soroban_sdk::Vec<u64> = soroban_sdk::Vec::new(env);
    for i in 0..active.len() {
        let id = active.get(i).unwrap();
        if let Some(dc) = get_contract(env, id) {
            if dc.status == DerivativeStatus::Active {
                let (under_a, under_b) = check_margin_health(&dc, cfg);
                if under_a || under_b {
                    result.push_back(id);
                }
            }
        }
    }
    result
}
