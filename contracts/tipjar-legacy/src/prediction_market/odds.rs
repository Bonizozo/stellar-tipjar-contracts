//! Odds and probability calculations for prediction markets.
//!
//! Uses a simple parimutuel model: the implied probability of each outcome
//! is proportional to the amount bet on it relative to the total pool.
//! Odds are expressed as a fixed-point multiplier with `ODDS_PRECISION` (1_000_000 = 1.0×).

use super::{Outcome, PredictionMarket, ODDS_PRECISION};

/// Returns the implied probability of `outcome` as a fraction of `ODDS_PRECISION`.
///
/// Returns `ODDS_PRECISION / 2` (50 %) when no bets have been placed yet.
pub fn implied_probability(market: &PredictionMarket, outcome: Outcome) -> i128 {
    let total = market.yes_pool + market.no_pool;
    if total == 0 {
        return ODDS_PRECISION / 2;
    }
    let side = match outcome {
        Outcome::None => return ODDS_PRECISION / 2,
        Outcome::Yes => market.yes_pool,
        Outcome::No => market.no_pool,
    };
    side * ODDS_PRECISION / total
}

/// Returns the payout multiplier for `outcome` as a fraction of `ODDS_PRECISION`.
///
/// A multiplier of `2_000_000` means a winning bet doubles its money (before fees).
/// Returns `2 * ODDS_PRECISION` when no bets have been placed yet.
pub fn payout_multiplier(market: &PredictionMarket, outcome: Outcome) -> i128 {
    let total = market.yes_pool + market.no_pool;
    let side = match outcome {
        Outcome::None => return 100 * ODDS_PRECISION,
        Outcome::Yes => market.yes_pool,
        Outcome::No => market.no_pool,
    };
    if side == 0 {
        // No bets on this side yet; theoretical multiplier is infinite, cap at 100×.
        return 100 * ODDS_PRECISION;
    }
    total * ODDS_PRECISION / side
}

/// Returns `(yes_probability, no_probability)` both scaled by `ODDS_PRECISION`.
pub fn market_odds(market: &PredictionMarket) -> (i128, i128) {
    (
        implied_probability(market, Outcome::Yes),
        implied_probability(market, Outcome::No),
    )
}
