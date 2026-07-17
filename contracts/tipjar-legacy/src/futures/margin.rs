//! Margin calculations for futures contracts.

use super::{FuturesContract, FuturesStatus, Side, BPS_DENOM, PRICE_PRECISION};
use soroban_sdk::Address;

/// Calculate the required initial margin for a contract.
///
/// `initial_margin = notional * initial_margin_bps / BPS_DENOM`
/// where `notional = size * contract_price / PRICE_PRECISION`.
pub fn required_initial_margin(size: i128, contract_price: i128, initial_margin_bps: u32) -> i128 {
    let notional = size * contract_price / PRICE_PRECISION;
    notional * initial_margin_bps as i128 / BPS_DENOM
}

/// Calculate the required maintenance margin for a contract.
pub fn required_maintenance_margin(
    size: i128,
    contract_price: i128,
    maintenance_margin_bps: u32,
) -> i128 {
    let notional = size * contract_price / PRICE_PRECISION;
    notional * maintenance_margin_bps as i128 / BPS_DENOM
}

/// Effective margin for the long side = posted margin + unrealised P&L.
pub fn long_effective_margin(fc: &FuturesContract) -> i128 {
    fc.long_margin + fc.long_unrealised_pnl
}

/// Effective margin for the short side = posted margin - unrealised P&L
/// (short profits when long loses).
pub fn short_effective_margin(fc: &FuturesContract) -> i128 {
    fc.short_margin - fc.long_unrealised_pnl
}

/// Returns `true` if the long side is below maintenance margin.
pub fn long_is_liquidatable(fc: &FuturesContract) -> bool {
    if fc.status != FuturesStatus::Active {
        return false;
    }
    let maint = required_maintenance_margin(fc.size, fc.contract_price, fc.maintenance_margin_bps);
    long_effective_margin(fc) < maint
}

/// Returns `true` if the short side is below maintenance margin.
pub fn short_is_liquidatable(fc: &FuturesContract) -> bool {
    if fc.status != FuturesStatus::Active || fc.short_party.is_none() {
        return false;
    }
    let maint = required_maintenance_margin(fc.size, fc.contract_price, fc.maintenance_margin_bps);
    short_effective_margin(fc) < maint
}

/// Returns `(side, party_address, margin_amount)` for the first liquidatable
/// side, or `(Side::Long, None, 0)` if neither side is liquidatable.
pub fn find_liquidatable_side(fc: &FuturesContract) -> (Side, Option<Address>, i128) {
    if long_is_liquidatable(fc) {
        return (Side::Long, Some(fc.long_party.clone()), fc.long_margin);
    }
    if short_is_liquidatable(fc) {
        let short = fc.short_party.clone();
        return (Side::Short, short, fc.short_margin);
    }
    (Side::Long, None, 0)
}
