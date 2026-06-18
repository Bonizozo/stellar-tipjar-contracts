//! Settlement and payout calculations for resolved prediction markets.

use soroban_sdk::Env;

use super::{BettorPosition, Outcome, PredictionMarket, BPS_DENOM};

/// Calculate the payout for a bettor in a resolved market.
///
/// Payout = (bettor's winning-side stake / total winning-side pool) × total pool × (1 - fee).
/// Returns 0 if the bettor had no stake on the winning side.
pub fn calculate_payout(_env: &Env, market: &PredictionMarket, position: &BettorPosition) -> i128 {
    let winning = match market.winning_outcome {
        Outcome::None => return 0,
        o => o,
    };

    let bettor_stake = match winning {
        Outcome::Yes => position.yes_amount,
        Outcome::No => position.no_amount,
        Outcome::None => return 0,
    };

    if bettor_stake == 0 {
        return 0;
    }

    let winning_pool = match winning {
        Outcome::Yes => market.yes_pool,
        Outcome::No => market.no_pool,
        Outcome::None => return 0,
    };

    if winning_pool == 0 {
        return 0;
    }

    let total_pool = market.yes_pool + market.no_pool;

    // Gross payout proportional to share of winning pool
    let gross = bettor_stake * total_pool / winning_pool;

    // Deduct platform fee
    let fee = gross * (market.fee_bps as i128) / (BPS_DENOM as i128);
    gross - fee
}

/// Calculate the total fee collected from a resolved market.
pub fn total_fee(market: &PredictionMarket) -> i128 {
    let total_pool = market.yes_pool + market.no_pool;
    total_pool * (market.fee_bps as i128) / (BPS_DENOM as i128)
}
