//! Cash settlement logic for futures contracts.
//!
//! At expiry the contract is settled against the final oracle price.
//! The net P&L is transferred between the two parties; remaining margin
//! is returned to each side.

use soroban_sdk::{token, Env};

use super::{
    get_contract, get_position, remove_active_contract, save_contract, save_position,
    FuturesStatus, Side, PRICE_PRECISION,
};

/// Settle a futures contract at the given final price.
///
/// Steps:
/// 1. Verify the contract is active and past its settlement date.
/// 2. Compute final P&L: `pnl = (final_price - contract_price) * size / PRICE_PRECISION`.
/// 3. Transfer `|pnl|` from the losing side's margin to the winning side.
/// 4. Return remaining margin to each party.
/// 5. Mark contract as `Settled`.
///
/// Returns `(long_payout, short_payout)`.
pub fn settle(env: &Env, contract_id: u64, final_price: i128) -> (i128, i128) {
    let mut fc = get_contract(env, contract_id).expect("contract not found");

    assert!(
        fc.status == FuturesStatus::Active || fc.status == FuturesStatus::PendingSettlement,
        "contract not settleable"
    );
    assert!(fc.short_party.is_some(), "contract not matched");
    assert!(
        env.ledger().timestamp() >= fc.settles_at,
        "settlement date not reached"
    );

    let short_party = fc.short_party.clone().unwrap();

    // Cash P&L for the long side
    let pnl = (final_price - fc.contract_price) * fc.size / PRICE_PRECISION;

    let (long_payout, short_payout) = if pnl >= 0 {
        // Long wins: transfer pnl from short margin to long
        let transfer = pnl.min(fc.short_margin);
        (fc.long_margin + transfer, fc.short_margin - transfer)
    } else {
        // Short wins: transfer |pnl| from long margin to short
        let transfer = (-pnl).min(fc.long_margin);
        (fc.long_margin - transfer, fc.short_margin + transfer)
    };

    let token_client = token::Client::new(env, &fc.token);

    if long_payout > 0 {
        token_client.transfer(
            &env.current_contract_address(),
            &fc.long_party,
            &long_payout,
        );
    }
    if short_payout > 0 {
        token_client.transfer(&env.current_contract_address(), &short_party, &short_payout);
    }

    // Update trader positions
    let mut long_pos = get_position(env, &fc.long_party);
    long_pos.long_count = long_pos.long_count.saturating_sub(1);
    long_pos.total_margin = long_pos.total_margin.saturating_sub(fc.long_margin);
    long_pos.realised_pnl += pnl;
    save_position(env, &long_pos);

    let mut short_pos = get_position(env, &short_party);
    short_pos.short_count = short_pos.short_count.saturating_sub(1);
    short_pos.total_margin = short_pos.total_margin.saturating_sub(fc.short_margin);
    short_pos.realised_pnl -= pnl; // short gains when long loses
    save_position(env, &short_pos);

    fc.status = FuturesStatus::Settled;
    fc.mark_price = final_price;
    fc.long_unrealised_pnl = pnl;
    save_contract(env, &fc);
    remove_active_contract(env, contract_id);

    (long_payout, short_payout)
}

/// Mark a contract as pending settlement once its settlement date has passed.
/// This is a lightweight state transition that can be called by anyone.
pub fn mark_pending(env: &Env, contract_id: u64) {
    let mut fc = get_contract(env, contract_id).expect("contract not found");
    assert!(fc.status == FuturesStatus::Active, "contract not active");
    assert!(
        env.ledger().timestamp() >= fc.settles_at,
        "settlement date not reached"
    );
    fc.status = FuturesStatus::PendingSettlement;
    save_contract(env, &fc);
}

/// Compute the expected payout for a given side at a hypothetical final price.
/// Useful for UI display without modifying state.
pub fn compute_payout(
    long_margin: i128,
    short_margin: i128,
    contract_price: i128,
    size: i128,
    final_price: i128,
    side: Side,
) -> i128 {
    let pnl = (final_price - contract_price) * size / PRICE_PRECISION;
    match side {
        Side::Long => {
            if pnl >= 0 {
                long_margin + pnl.min(short_margin)
            } else {
                (long_margin - (-pnl).min(long_margin)).max(0)
            }
        }
        Side::Short => {
            if pnl <= 0 {
                short_margin + (-pnl).min(long_margin)
            } else {
                (short_margin - pnl.min(short_margin)).max(0)
            }
        }
    }
}
