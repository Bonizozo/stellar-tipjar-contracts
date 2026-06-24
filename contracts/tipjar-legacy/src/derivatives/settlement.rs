//! Settlement engine for the derivatives platform.
//!
//! Handles:
//! - **Option exercise**: holder exercises before expiry (American-style allowed).
//! - **Expiry settlement**: cash-settle all expired contracts against the final
//!   oracle price.
//! - **Batch expiry**: process multiple expired contracts in one call.
//!
//! All settlements are cash-settled (no physical delivery of tokens).
//! Net P&L is transferred between the two parties; remaining collateral is
//! returned to each party.

use super::{
    DerivativeContract, DerivativeKind, DerivativeStatus,
    get_contract, get_portfolio, remove_active, save_contract, save_portfolio,
    PRICE_PRECISION,
};
use soroban_sdk::{token, Address, Env};

// ── Option exercise ──────────────────────────────────────────────────────────

/// Exercise an option contract. Only the holder (`party_b`) may call this.
/// The contract must be Active and not yet expired.
///
/// Cash settlement: the intrinsic value is transferred from party_a to party_b.
/// Both parties receive their remaining collateral back.
pub fn exercise_option(env: &Env, holder: &Address, id: u64) {
    let mut dc = get_contract(env, id).expect("contract not found");
    assert!(dc.status == DerivativeStatus::Active, "contract not active");
    assert!(
        dc.kind == DerivativeKind::Call || dc.kind == DerivativeKind::Put,
        "not an option"
    );
    let party_b = dc.party_b.clone().expect("no counterparty");
    assert!(party_b == *holder, "only the holder can exercise");
    assert!(
        env.ledger().timestamp() <= dc.expires_at,
        "option has expired"
    );

    let intrinsic = intrinsic_value(&dc, dc.mark_price);
    assert!(intrinsic > 0, "option is out of the money");

    let token_client = token::Client::new(env, &dc.token);

    // Pay intrinsic value from party_a's collateral to party_b
    let payout = intrinsic.min(dc.collateral_a);
    if payout > 0 {
        token_client.transfer(&env.current_contract_address(), holder, &payout);
    }

    // Return remaining collateral to party_a
    let remaining_a = dc.collateral_a - payout;
    if remaining_a > 0 {
        token_client.transfer(&env.current_contract_address(), &dc.party_a, &remaining_a);
    }

    // Return party_b's collateral
    if dc.collateral_b > 0 {
        token_client.transfer(&env.current_contract_address(), holder, &dc.collateral_b);
    }

    _update_portfolios_on_close(env, &dc, payout, -payout);

    dc.settlement_price = dc.mark_price;
    dc.status = DerivativeStatus::Exercised;
    save_contract(env, &dc);
    remove_active(env, id);
}

// ── Expiry settlement ────────────────────────────────────────────────────────

/// Settle a single expired contract at the given final price.
/// Can be called by anyone once `expires_at` has passed.
///
/// For futures/swaps: net P&L is transferred between parties.
/// For options: if in-the-money, intrinsic value is paid to party_b; otherwise
/// the contract expires worthless and collateral is returned.
pub fn settle_expired(env: &Env, id: u64, final_price: i128) {
    let mut dc = get_contract(env, id).expect("contract not found");
    assert!(dc.status == DerivativeStatus::Active, "contract not active");
    assert!(
        env.ledger().timestamp() > dc.expires_at,
        "contract not yet expired"
    );
    assert!(final_price > 0, "invalid final price");

    dc.settlement_price = final_price;
    dc.mark_price = final_price;

    let token_client = token::Client::new(env, &dc.token);

    match dc.kind {
        DerivativeKind::Call | DerivativeKind::Put => {
            let intrinsic = intrinsic_value(&dc, final_price);
            if intrinsic > 0 {
                // In-the-money: pay intrinsic to party_b from party_a's collateral
                let party_b = dc.party_b.clone().expect("no party_b");
                let payout = intrinsic.min(dc.collateral_a);
                if payout > 0 {
                    token_client.transfer(&env.current_contract_address(), &party_b, &payout);
                }
                let remaining_a = dc.collateral_a - payout;
                if remaining_a > 0 {
                    token_client.transfer(
                        &env.current_contract_address(),
                        &dc.party_a,
                        &remaining_a,
                    );
                }
                if dc.collateral_b > 0 {
                    token_client.transfer(
                        &env.current_contract_address(),
                        &party_b,
                        &dc.collateral_b,
                    );
                }
                _update_portfolios_on_close(env, &dc, -payout, payout);
                dc.status = DerivativeStatus::Settled;
            } else {
                // Out-of-the-money: return all collateral
                _return_all_collateral(env, &dc, &token_client);
                dc.status = DerivativeStatus::Expired;
            }
        }
        DerivativeKind::Future | DerivativeKind::Swap => {
            // Cash-settle: long P&L = (final - strike) * notional / PRICE_PRECISION
            let pnl_a = (final_price - dc.strike) * dc.notional / PRICE_PRECISION;
            _settle_futures_pnl(env, &dc, &token_client, pnl_a);
            dc.status = DerivativeStatus::Settled;
        }
    }

    save_contract(env, &dc);
    remove_active(env, id);
}

/// Batch-settle all expired contracts from the active list.
/// Returns the number of contracts settled.
pub fn settle_all_expired(env: &Env, final_prices: &[(u64, i128)]) -> u32 {
    // Build a lookup: contract_id → final_price
    // Since we can't use std HashMap in no_std, iterate linearly (list is bounded).
    let active = super::get_active_ids(env);
    let mut count: u32 = 0;
    let now = env.ledger().timestamp();

    for i in 0..active.len() {
        let id = active.get(i).unwrap();
        if let Some(dc) = get_contract(env, id) {
            if dc.status == DerivativeStatus::Active && now > dc.expires_at {
                // Find the matching final price
                let mut price_opt: Option<i128> = None;
                for (cid, price) in final_prices.iter() {
                    if *cid == id {
                        price_opt = Some(*price);
                        break;
                    }
                }
                if let Some(price) = price_opt {
                    settle_expired(env, id, price);
                    count += 1;
                }
            }
        }
    }
    count
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Compute the intrinsic value of an option at a given price.
fn intrinsic_value(dc: &DerivativeContract, price: i128) -> i128 {
    match dc.kind {
        DerivativeKind::Call => {
            let diff = price - dc.strike;
            if diff > 0 {
                diff * dc.notional / PRICE_PRECISION
            } else {
                0
            }
        }
        DerivativeKind::Put => {
            let diff = dc.strike - price;
            if diff > 0 {
                diff * dc.notional / PRICE_PRECISION
            } else {
                0
            }
        }
        _ => 0,
    }
}

/// Settle a futures/swap contract given the long-side P&L.
fn _settle_futures_pnl(
    env: &Env,
    dc: &DerivativeContract,
    token_client: &token::Client,
    pnl_a: i128,
) {
    let party_b = dc.party_b.clone().expect("no party_b");

    if pnl_a >= 0 {
        // party_a profits: transfer pnl_a from party_b's collateral to party_a
        let transfer = pnl_a.min(dc.collateral_b);
        if transfer > 0 {
            token_client.transfer(&env.current_contract_address(), &dc.party_a, &transfer);
        }
        // Return remaining collateral
        let rem_a = dc.collateral_a;
        let rem_b = dc.collateral_b - transfer;
        if rem_a > 0 {
            token_client.transfer(&env.current_contract_address(), &dc.party_a, &rem_a);
        }
        if rem_b > 0 {
            token_client.transfer(&env.current_contract_address(), &party_b, &rem_b);
        }
        _update_portfolios_on_close(env, dc, pnl_a, -pnl_a);
    } else {
        // party_b profits: transfer |pnl_a| from party_a's collateral to party_b
        let loss_a = (-pnl_a).min(dc.collateral_a);
        if loss_a > 0 {
            token_client.transfer(&env.current_contract_address(), &party_b, &loss_a);
        }
        let rem_a = dc.collateral_a - loss_a;
        let rem_b = dc.collateral_b;
        if rem_a > 0 {
            token_client.transfer(&env.current_contract_address(), &dc.party_a, &rem_a);
        }
        if rem_b > 0 {
            token_client.transfer(&env.current_contract_address(), &party_b, &rem_b);
        }
        _update_portfolios_on_close(env, dc, pnl_a, -pnl_a);
    }
}

/// Return all collateral to both parties (used for expired worthless options).
fn _return_all_collateral(
    env: &Env,
    dc: &DerivativeContract,
    token_client: &token::Client,
) {
    if dc.collateral_a > 0 {
        token_client.transfer(&env.current_contract_address(), &dc.party_a, &dc.collateral_a);
    }
    if let Some(ref pb) = dc.party_b {
        if dc.collateral_b > 0 {
            token_client.transfer(&env.current_contract_address(), pb, &dc.collateral_b);
        }
    }
    _update_portfolios_on_close(env, dc, 0, 0);
}

/// Update portfolio counters and realised P&L after a contract closes.
fn _update_portfolios_on_close(env: &Env, dc: &DerivativeContract, pnl_a: i128, pnl_b: i128) {
    let mut pa = get_portfolio(env, &dc.party_a);
    pa.initiated_count = pa.initiated_count.saturating_sub(1);
    pa.total_collateral = pa.total_collateral.saturating_sub(dc.collateral_a);
    pa.realised_pnl += pnl_a;
    save_portfolio(env, &pa);

    if let Some(ref pb_addr) = dc.party_b {
        let mut pb = get_portfolio(env, pb_addr);
        pb.matched_count = pb.matched_count.saturating_sub(1);
        pb.total_collateral = pb.total_collateral.saturating_sub(dc.collateral_b);
        pb.realised_pnl += pnl_b;
        save_portfolio(env, &pb);
    }
}
