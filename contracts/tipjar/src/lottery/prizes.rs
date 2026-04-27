//! Prize pool calculation and distribution.
//!
//! Prize distribution uses a tiered model:
//! - 1 winner:  100% of pool
//! - 2 winners: 70% / 30%
//! - 3 winners: 60% / 25% / 15%
//! - 4+ winners: 50% to 1st, remainder split equally among the rest
//!
//! Admins fund the prize pool when creating a round. Winners claim their
//! prizes individually; unclaimed prizes can be reclaimed by admin after
//! a configurable expiry.

use soroban_sdk::{panic_with_error, token, Address, Env};

use crate::{DataKey, TipJarError};

use super::{drawing::get_winner, LotteryStatus, LotteryWinner, MAX_WINNERS_PER_ROUND};

// ── Prize distribution ───────────────────────────────────────────────────────

/// Calculates prize amounts for each winner. Returns amounts indexed 0..winner_count.
///
/// Distribution tiers (in basis points, must sum to 10 000):
/// - 1 winner:  [10_000]
/// - 2 winners: [7_000, 3_000]
/// - 3 winners: [6_000, 2_500, 1_500]
/// - 4+ winners: [5_000, then remainder / (n-1) each]
pub fn calculate_prize_amounts(prize_pool: i128, winner_count: u32) -> [i128; 10] {
    let mut amounts = [0i128; 10];
    if winner_count == 0 || prize_pool <= 0 {
        return amounts;
    }

    let n = winner_count.min(MAX_WINNERS_PER_ROUND) as usize;

    match n {
        1 => {
            amounts[0] = prize_pool;
        }
        2 => {
            amounts[0] = prize_pool * 7_000 / 10_000;
            amounts[1] = prize_pool - amounts[0]; // remainder to 2nd
        }
        3 => {
            amounts[0] = prize_pool * 6_000 / 10_000;
            amounts[1] = prize_pool * 2_500 / 10_000;
            amounts[2] = prize_pool - amounts[0] - amounts[1]; // remainder to 3rd
        }
        _ => {
            // 1st place gets 50%, rest split equally
            amounts[0] = prize_pool * 5_000 / 10_000;
            let remainder = prize_pool - amounts[0];
            let per_winner = remainder / (n as i128 - 1);
            let mut distributed = amounts[0];
            for i in 1..n - 1 {
                amounts[i] = per_winner;
                distributed += per_winner;
            }
            // Last winner gets the remainder to avoid rounding loss
            amounts[n - 1] = prize_pool - distributed;
        }
    }

    amounts
}

// ── Prize claiming ───────────────────────────────────────────────────────────

/// Claims the prize for a winner in a completed round.
///
/// Transfers the prize amount from the contract to the winner.
/// Marks the winner record as claimed.
///
/// Panics if:
/// - Round not found or not completed.
/// - Caller is not the winner at `position`.
/// - Prize already claimed.
pub fn claim_prize(env: &Env, winner: &Address, round_id: u64, position: u32) {
    winner.require_auth();

    let round: super::LotteryRound = env
        .storage()
        .persistent()
        .get(&DataKey::LotteryRound(round_id))
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::LotteryRoundNotFound));

    if round.status != LotteryStatus::Completed {
        panic_with_error!(env, TipJarError::LotteryRoundNotCompleted);
    }

    let mut winner_record: LotteryWinner = env
        .storage()
        .persistent()
        .get(&DataKey::LotteryWinner(round_id, position))
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::LotteryWinnerNotFound));

    if winner_record.winner != *winner {
        panic_with_error!(env, TipJarError::Unauthorized);
    }

    if winner_record.claimed_at.is_some() {
        panic_with_error!(env, TipJarError::LotteryPrizeAlreadyClaimed);
    }

    let prize = winner_record.prize_amount;
    if prize <= 0 {
        panic_with_error!(env, TipJarError::NothingToWithdraw);
    }

    // Mark as claimed
    winner_record.claimed_at = Some(env.ledger().timestamp());
    env.storage()
        .persistent()
        .set(&DataKey::LotteryWinner(round_id, position), &winner_record);

    // Transfer prize to winner
    token::Client::new(env, &round.prize_token).transfer(
        &env.current_contract_address(),
        winner,
        &prize,
    );

    env.events().publish(
        (soroban_sdk::symbol_short!("lot_clm"),),
        (winner.clone(), round_id, position, prize),
    );
}

/// Reclaims unclaimed prizes from a completed round back to admin.
///
/// Can only be called by admin. Transfers all unclaimed prize amounts back.
/// Returns the total amount reclaimed.
pub fn reclaim_unclaimed_prizes(env: &Env, admin: &Address, round_id: u64) -> i128 {
    admin.require_auth();
    require_admin(env, admin);

    let round: super::LotteryRound = env
        .storage()
        .persistent()
        .get(&DataKey::LotteryRound(round_id))
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::LotteryRoundNotFound));

    if round.status != LotteryStatus::Completed {
        panic_with_error!(env, TipJarError::LotteryRoundNotCompleted);
    }

    let mut total_reclaimed: i128 = 0;

    for position in 1..=round.winner_count {
        if let Some(mut winner_record) = get_winner(env, round_id, position) {
            if winner_record.claimed_at.is_none() && winner_record.prize_amount > 0 {
                total_reclaimed += winner_record.prize_amount;
                winner_record.claimed_at = Some(env.ledger().timestamp());
                env.storage().persistent().set(
                    &DataKey::LotteryWinner(round_id, position),
                    &winner_record,
                );
            }
        }
    }

    if total_reclaimed > 0 {
        token::Client::new(env, &round.prize_token).transfer(
            &env.current_contract_address(),
            admin,
            &total_reclaimed,
        );

        env.events().publish(
            (soroban_sdk::symbol_short!("lot_rclm"),),
            (admin.clone(), round_id, total_reclaimed),
        );
    }

    total_reclaimed
}

// ── Internal helpers ─────────────────────────────────────────────────────────

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
