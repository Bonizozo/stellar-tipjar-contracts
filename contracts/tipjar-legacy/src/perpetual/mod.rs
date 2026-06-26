//! Perpetual Swap Contracts
//!
//! Provides leveraged tip trading without expiration, including funding rates,
//! leverage, liquidations, unrealized PnL, and position tracking.

use soroban_sdk::{contracttype, symbol_short, Address, Env};

/// Direction of a perpetual swap position.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PositionSide {
    Long,
    Short,
}

/// A perpetual swap position held by a trader.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapPosition {
    pub trader: Address,
    /// Notional size in base units.
    pub size: i128,
    /// Entry price scaled by 1_000_000.
    pub entry_price: i128,
    /// Collateral posted (margin).
    pub collateral: i128,
    /// Cumulative funding index already settled into this position.
    pub funding_settled: i128,
    pub side: PositionSide,
    pub opened_at: u64,
}

/// Global funding-rate state for the perpetual market.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundingState {
    /// Cumulative funding index (scaled by 1_000_000).
    pub cumulative_index: i128,
    /// Last time funding was updated (ledger timestamp).
    pub last_updated: u64,
    /// Current funding rate per second (scaled by 1_000_000_000).
    pub rate_per_second: i128,
}

// ---------------------------------------------------------------------------
// Funding
// ---------------------------------------------------------------------------

/// Accrue funding since the last update and return the updated state.
pub fn accrue_funding(env: &Env, mark_price: i128) -> FundingState {
    let key = symbol_short!("funding");
    let mut state: FundingState = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(FundingState {
            cumulative_index: 0,
            last_updated: env.ledger().timestamp(),
            rate_per_second: 0,
        });

    let now = env.ledger().timestamp();
    let elapsed = (now - state.last_updated) as i128;

    if elapsed > 0 {
        // Daily rate = 0.1% of mark price; rate_per_second = daily_rate / 86_400
        let daily_rate = mark_price / 1_000;
        state.rate_per_second = daily_rate / 86_400;
        state.cumulative_index += state.rate_per_second * elapsed;
        state.last_updated = now;
        env.storage().persistent().set(&key, &state);
    }

    state
}

// ---------------------------------------------------------------------------
// Positions
// ---------------------------------------------------------------------------

/// Open a new perpetual swap position.
///
/// `leverage` is an integer multiplier (e.g. 10 = 10×, max 100×).
pub fn open_position(
    env: &Env,
    trader: &Address,
    collateral: i128,
    leverage: i128,
    mark_price: i128,
    side: PositionSide,
) -> SwapPosition {
    trader.require_auth();

    assert!(collateral > 0, "collateral must be positive");
    assert!(leverage >= 1 && leverage <= 100, "leverage 1-100");
    assert!(mark_price > 0, "invalid mark price");

    let funding = accrue_funding(env, mark_price);
    let size = collateral * leverage;

    let pos = SwapPosition {
        trader: trader.clone(),
        size,
        entry_price: mark_price,
        collateral,
        funding_settled: funding.cumulative_index,
        side,
        opened_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&(symbol_short!("pos"), trader.clone()), &pos);

    env.events().publish(
        (symbol_short!("perp"), symbol_short!("open")),
        (trader.clone(), size, mark_price),
    );

    pos
}

/// Close an existing position and return unrealized PnL.
pub fn close_position(env: &Env, trader: &Address, mark_price: i128) -> i128 {
    trader.require_auth();

    let pos: SwapPosition = env
        .storage()
        .persistent()
        .get(&(symbol_short!("pos"), trader.clone()))
        .expect("no open position");

    let funding = accrue_funding(env, mark_price);
    let pnl = calculate_pnl(&pos, mark_price, funding.cumulative_index);

    env.storage()
        .persistent()
        .remove(&(symbol_short!("pos"), trader.clone()));

    env.events().publish(
        (symbol_short!("perp"), symbol_short!("close")),
        (trader.clone(), pnl),
    );

    pnl
}

/// Calculate unrealized PnL including pending funding payments.
pub fn calculate_pnl(pos: &SwapPosition, mark_price: i128, cumulative_index: i128) -> i128 {
    let price_pnl = match pos.side {
        PositionSide::Long => (mark_price - pos.entry_price) * pos.size / pos.entry_price,
        PositionSide::Short => (pos.entry_price - mark_price) * pos.size / pos.entry_price,
    };
    let funding_payment = (cumulative_index - pos.funding_settled) * pos.size / 1_000_000;
    price_pnl - funding_payment
}

/// Returns `true` when remaining margin falls below 5% of notional.
pub fn is_liquidatable(pos: &SwapPosition, mark_price: i128, cumulative_index: i128) -> bool {
    let pnl = calculate_pnl(pos, mark_price, cumulative_index);
    let remaining_margin = pos.collateral + pnl;
    let maintenance_margin = pos.size / 20; // 5%
    remaining_margin < maintenance_margin
}

/// Liquidate an under-margined position.
pub fn liquidate(env: &Env, trader: &Address, mark_price: i128) {
    let pos: SwapPosition = env
        .storage()
        .persistent()
        .get(&(symbol_short!("pos"), trader.clone()))
        .expect("no open position");

    let funding = accrue_funding(env, mark_price);
    assert!(
        is_liquidatable(&pos, mark_price, funding.cumulative_index),
        "position not liquidatable"
    );

    env.storage()
        .persistent()
        .remove(&(symbol_short!("pos"), trader.clone()));

    env.events().publish(
        (symbol_short!("perp"), symbol_short!("liq")),
        (trader.clone(), mark_price),
    );
}

/// Get an open position for a trader, if any.
pub fn get_position(env: &Env, trader: &Address) -> Option<SwapPosition> {
    env.storage()
        .persistent()
        .get(&(symbol_short!("pos"), trader.clone()))
}
