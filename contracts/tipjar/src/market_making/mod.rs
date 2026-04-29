//! Tip Market Making System
//!
//! Provides automated market making for tip tokens: market makers post bid/ask
//! quotes, earn fees from matched trades, and manage inventory risk.

use soroban_sdk::{contracttype, panic_with_error, symbol_short, token, Address, Env, Vec};

// ── Types ────────────────────────────────────────────────────────────────────

/// Status of a market maker position.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum MakerStatus {
    Active,
    Paused,
    Closed,
}

/// A market maker's configuration and live state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketMaker {
    pub maker_id: u64,
    pub maker: Address,
    /// Base token (e.g. tip token).
    pub base_token: Address,
    /// Quote token (e.g. USDC).
    pub quote_token: Address,
    /// Spread in basis points (e.g. 50 = 0.5%).
    pub spread_bps: u32,
    /// Base token inventory deposited by the maker.
    pub base_inventory: i128,
    /// Quote token inventory deposited by the maker.
    pub quote_inventory: i128,
    /// Maximum single-trade size in base token units.
    pub max_trade_size: i128,
    /// Accumulated fees earned in quote token.
    pub fees_earned: i128,
    /// Total base volume traded.
    pub total_base_volume: i128,
    /// Total quote volume traded.
    pub total_quote_volume: i128,
    /// Number of trades executed.
    pub trade_count: u64,
    pub status: MakerStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

/// A quote posted by a market maker (bid or ask).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Quote {
    pub maker_id: u64,
    /// Mid-price used to derive bid/ask (× PRICE_PRECISION).
    pub mid_price: i128,
    /// Bid price = mid_price × (1 - spread/2).
    pub bid_price: i128,
    /// Ask price = mid_price × (1 + spread/2).
    pub ask_price: i128,
    /// Maximum base amount available at this quote.
    pub available_base: i128,
    /// Maximum quote amount available at this quote.
    pub available_quote: i128,
    pub timestamp: u64,
}

/// Result of a trade against a market maker.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TradeResult {
    pub maker_id: u64,
    pub base_amount: i128,
    pub quote_amount: i128,
    pub fee_amount: i128,
    pub price: i128,
    pub timestamp: u64,
}

/// Storage sub-keys for market making data.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarketMakingKey {
    /// MarketMaker record keyed by maker_id.
    Maker(u64),
    /// Global maker ID counter.
    Counter,
    /// List of maker IDs owned by an address.
    OwnerMakers(Address),
    /// List of all active maker IDs.
    ActiveMakers,
    /// Maker IDs for a token pair (base, quote).
    PairMakers(Address, Address),
}

// ── Constants ────────────────────────────────────────────────────────────────

pub const PRICE_PRECISION: i128 = 1_000_000;
/// Maximum spread: 10% (1000 bps).
pub const MAX_SPREAD_BPS: u32 = 1_000;
/// Fee charged per trade: 0.1% (10 bps) of quote amount.
pub const TRADE_FEE_BPS: u32 = 10;

// ── Errors ───────────────────────────────────────────────────────────────────

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MarketMakingError {
    MakerNotFound = 800,
    MakerNotActive = 801,
    Unauthorized = 802,
    InvalidSpread = 803,
    InvalidInventory = 804,
    InvalidTradeSize = 805,
    InsufficientBaseInventory = 806,
    InsufficientQuoteInventory = 807,
    TradeSizeExceedsMax = 808,
    TokenNotWhitelisted = 809,
    IdenticalTokens = 810,
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Registers a new market maker and deposits initial inventory.
///
/// Returns the new maker_id.
/// Emits `("mm_reg",)` with data `(maker_id, maker, base_token, quote_token, spread_bps)`.
pub fn register_maker(
    env: &Env,
    maker: &Address,
    base_token: &Address,
    quote_token: &Address,
    spread_bps: u32,
    base_deposit: i128,
    quote_deposit: i128,
    max_trade_size: i128,
) -> u64 {
    maker.require_auth();

    if base_token == quote_token {
        panic_with_error!(env, MarketMakingError::IdenticalTokens);
    }
    if spread_bps == 0 || spread_bps > MAX_SPREAD_BPS {
        panic_with_error!(env, MarketMakingError::InvalidSpread);
    }
    if base_deposit < 0 || quote_deposit < 0 || (base_deposit == 0 && quote_deposit == 0) {
        panic_with_error!(env, MarketMakingError::InvalidInventory);
    }
    if max_trade_size <= 0 {
        panic_with_error!(env, MarketMakingError::InvalidTradeSize);
    }

    let maker_id: u64 = env
        .storage()
        .instance()
        .get(&crate::DataKey::MarketMaking(MarketMakingKey::Counter))
        .unwrap_or(0u64);
    env.storage()
        .instance()
        .set(&crate::DataKey::MarketMaking(MarketMakingKey::Counter), &(maker_id + 1));

    let now = env.ledger().timestamp();
    let mm = MarketMaker {
        maker_id,
        maker: maker.clone(),
        base_token: base_token.clone(),
        quote_token: quote_token.clone(),
        spread_bps,
        base_inventory: base_deposit,
        quote_inventory: quote_deposit,
        max_trade_size,
        fees_earned: 0,
        total_base_volume: 0,
        total_quote_volume: 0,
        trade_count: 0,
        status: MakerStatus::Active,
        created_at: now,
        updated_at: now,
    };

    env.storage()
        .persistent()
        .set(&crate::DataKey::MarketMaking(MarketMakingKey::Maker(maker_id)), &mm);

    _add_owner_maker(env, maker, maker_id);
    _add_active_maker(env, maker_id);
    _add_pair_maker(env, base_token, quote_token, maker_id);

    // Transfer inventory into contract escrow
    if base_deposit > 0 {
        token::Client::new(env, base_token).transfer(
            maker,
            &env.current_contract_address(),
            &base_deposit,
        );
    }
    if quote_deposit > 0 {
        token::Client::new(env, quote_token).transfer(
            maker,
            &env.current_contract_address(),
            &quote_deposit,
        );
    }

    env.events().publish(
        (symbol_short!("mm_reg"),),
        (maker_id, maker.clone(), base_token.clone(), quote_token.clone(), spread_bps),
    );

    maker_id
}

/// Computes the current bid/ask quote for a market maker given a mid-price.
///
/// Does not modify state; purely a view function.
pub fn get_quote(env: &Env, maker_id: u64, mid_price: i128) -> Quote {
    let mm: MarketMaker = _get_maker_or_panic(env, maker_id);

    if mm.status != MakerStatus::Active {
        panic_with_error!(env, MarketMakingError::MakerNotActive);
    }

    let half_spread = (mid_price * mm.spread_bps as i128) / (2 * 10_000);
    let bid_price = mid_price.saturating_sub(half_spread);
    let ask_price = mid_price.saturating_add(half_spread);

    // Available base at ask, available quote at bid
    let available_base = mm.base_inventory.min(mm.max_trade_size);
    let available_quote = mm.quote_inventory.min(
        (mm.max_trade_size * bid_price) / PRICE_PRECISION,
    );

    Quote {
        maker_id,
        mid_price,
        bid_price,
        ask_price,
        available_base,
        available_quote,
        timestamp: env.ledger().timestamp(),
    }
}

/// Executes a buy trade against a market maker (taker buys base, pays quote).
///
/// `base_amount` — amount of base token the taker wants to buy.
/// `max_quote`   — maximum quote tokens the taker is willing to pay (slippage guard).
///
/// Emits `("mm_buy",)` with data `(maker_id, taker, base_amount, quote_paid, fee)`.
pub fn execute_buy(
    env: &Env,
    taker: &Address,
    maker_id: u64,
    base_amount: i128,
    mid_price: i128,
    max_quote: i128,
) -> TradeResult {
    taker.require_auth();

    let mut mm: MarketMaker = _get_maker_or_panic(env, maker_id);

    if mm.status != MakerStatus::Active {
        panic_with_error!(env, MarketMakingError::MakerNotActive);
    }
    if base_amount <= 0 || base_amount > mm.max_trade_size {
        panic_with_error!(env, MarketMakingError::TradeSizeExceedsMax);
    }
    if mm.base_inventory < base_amount {
        panic_with_error!(env, MarketMakingError::InsufficientBaseInventory);
    }

    // Ask price = mid × (1 + spread/2)
    let half_spread = (mid_price * mm.spread_bps as i128) / (2 * 10_000);
    let ask_price = mid_price.saturating_add(half_spread);
    let quote_needed = (base_amount * ask_price) / PRICE_PRECISION;
    let fee = (quote_needed * TRADE_FEE_BPS as i128) / 10_000;
    let total_quote = quote_needed + fee;

    if total_quote > max_quote {
        panic_with_error!(env, MarketMakingError::InsufficientQuoteInventory);
    }

    // State updates (CEI)
    mm.base_inventory -= base_amount;
    mm.quote_inventory += quote_needed;
    mm.fees_earned += fee;
    mm.total_base_volume += base_amount;
    mm.total_quote_volume += quote_needed;
    mm.trade_count += 1;
    mm.updated_at = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&crate::DataKey::MarketMaking(MarketMakingKey::Maker(maker_id)), &mm);

    // Transfers: taker pays quote, receives base
    token::Client::new(env, &mm.quote_token).transfer(
        taker,
        &env.current_contract_address(),
        &total_quote,
    );
    token::Client::new(env, &mm.base_token).transfer(
        &env.current_contract_address(),
        taker,
        &base_amount,
    );

    let result = TradeResult {
        maker_id,
        base_amount,
        quote_amount: total_quote,
        fee_amount: fee,
        price: ask_price,
        timestamp: mm.updated_at,
    };

    env.events().publish(
        (symbol_short!("mm_buy"),),
        (maker_id, taker.clone(), base_amount, total_quote, fee),
    );

    result
}

/// Executes a sell trade against a market maker (taker sells base, receives quote).
///
/// `base_amount` — amount of base token the taker wants to sell.
/// `min_quote`   — minimum quote tokens the taker expects to receive (slippage guard).
///
/// Emits `("mm_sell",)` with data `(maker_id, taker, base_amount, quote_received, fee)`.
pub fn execute_sell(
    env: &Env,
    taker: &Address,
    maker_id: u64,
    base_amount: i128,
    mid_price: i128,
    min_quote: i128,
) -> TradeResult {
    taker.require_auth();

    let mut mm: MarketMaker = _get_maker_or_panic(env, maker_id);

    if mm.status != MakerStatus::Active {
        panic_with_error!(env, MarketMakingError::MakerNotActive);
    }
    if base_amount <= 0 || base_amount > mm.max_trade_size {
        panic_with_error!(env, MarketMakingError::TradeSizeExceedsMax);
    }

    // Bid price = mid × (1 - spread/2)
    let half_spread = (mid_price * mm.spread_bps as i128) / (2 * 10_000);
    let bid_price = mid_price.saturating_sub(half_spread);
    let quote_gross = (base_amount * bid_price) / PRICE_PRECISION;
    let fee = (quote_gross * TRADE_FEE_BPS as i128) / 10_000;
    let quote_net = quote_gross.saturating_sub(fee);

    if mm.quote_inventory < quote_gross {
        panic_with_error!(env, MarketMakingError::InsufficientQuoteInventory);
    }
    if quote_net < min_quote {
        panic_with_error!(env, MarketMakingError::InsufficientQuoteInventory);
    }

    // State updates (CEI)
    mm.base_inventory += base_amount;
    mm.quote_inventory -= quote_gross;
    mm.fees_earned += fee;
    mm.total_base_volume += base_amount;
    mm.total_quote_volume += quote_gross;
    mm.trade_count += 1;
    mm.updated_at = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&crate::DataKey::MarketMaking(MarketMakingKey::Maker(maker_id)), &mm);

    // Transfers: taker pays base, receives quote
    token::Client::new(env, &mm.base_token).transfer(
        taker,
        &env.current_contract_address(),
        &base_amount,
    );
    token::Client::new(env, &mm.quote_token).transfer(
        &env.current_contract_address(),
        taker,
        &quote_net,
    );

    let result = TradeResult {
        maker_id,
        base_amount,
        quote_amount: quote_net,
        fee_amount: fee,
        price: bid_price,
        timestamp: mm.updated_at,
    };

    env.events().publish(
        (symbol_short!("mm_sell"),),
        (maker_id, taker.clone(), base_amount, quote_net, fee),
    );

    result
}

/// Adds more inventory to an existing market maker position.
///
/// Only the maker owner may call this.
/// Emits `("mm_dep",)` with data `(maker_id, base_added, quote_added)`.
pub fn deposit_inventory(
    env: &Env,
    maker: &Address,
    maker_id: u64,
    base_amount: i128,
    quote_amount: i128,
) {
    maker.require_auth();

    let mut mm: MarketMaker = _get_maker_or_panic(env, maker_id);

    if mm.maker != *maker {
        panic_with_error!(env, MarketMakingError::Unauthorized);
    }
    if mm.status == MakerStatus::Closed {
        panic_with_error!(env, MarketMakingError::MakerNotActive);
    }
    if base_amount < 0 || quote_amount < 0 || (base_amount == 0 && quote_amount == 0) {
        panic_with_error!(env, MarketMakingError::InvalidInventory);
    }

    mm.base_inventory += base_amount;
    mm.quote_inventory += quote_amount;
    mm.updated_at = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&crate::DataKey::MarketMaking(MarketMakingKey::Maker(maker_id)), &mm);

    if base_amount > 0 {
        token::Client::new(env, &mm.base_token).transfer(
            maker,
            &env.current_contract_address(),
            &base_amount,
        );
    }
    if quote_amount > 0 {
        token::Client::new(env, &mm.quote_token).transfer(
            maker,
            &env.current_contract_address(),
            &quote_amount,
        );
    }

    env.events()
        .publish((symbol_short!("mm_dep"),), (maker_id, base_amount, quote_amount));
}

/// Withdraws inventory and accumulated fees from a market maker position.
///
/// Only the maker owner may call this.
/// Emits `("mm_wdr",)` with data `(maker_id, base_withdrawn, quote_withdrawn, fees_withdrawn)`.
pub fn withdraw_inventory(
    env: &Env,
    maker: &Address,
    maker_id: u64,
    base_amount: i128,
    quote_amount: i128,
) {
    maker.require_auth();

    let mut mm: MarketMaker = _get_maker_or_panic(env, maker_id);

    if mm.maker != *maker {
        panic_with_error!(env, MarketMakingError::Unauthorized);
    }
    if base_amount > mm.base_inventory {
        panic_with_error!(env, MarketMakingError::InsufficientBaseInventory);
    }
    if quote_amount > mm.quote_inventory + mm.fees_earned {
        panic_with_error!(env, MarketMakingError::InsufficientQuoteInventory);
    }

    // Withdraw fees first, then inventory
    let mut fees_withdrawn: i128 = 0;
    let mut quote_from_inventory = quote_amount;
    if mm.fees_earned > 0 && quote_amount > 0 {
        fees_withdrawn = mm.fees_earned.min(quote_amount);
        mm.fees_earned -= fees_withdrawn;
        quote_from_inventory = quote_amount - fees_withdrawn;
    }

    mm.base_inventory -= base_amount;
    mm.quote_inventory -= quote_from_inventory;
    mm.updated_at = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&crate::DataKey::MarketMaking(MarketMakingKey::Maker(maker_id)), &mm);

    if base_amount > 0 {
        token::Client::new(env, &mm.base_token).transfer(
            &env.current_contract_address(),
            maker,
            &base_amount,
        );
    }
    if quote_amount > 0 {
        token::Client::new(env, &mm.quote_token).transfer(
            &env.current_contract_address(),
            maker,
            &quote_amount,
        );
    }

    env.events().publish(
        (symbol_short!("mm_wdr"),),
        (maker_id, base_amount, quote_amount, fees_withdrawn),
    );
}

/// Updates the spread for a market maker. Only the maker owner may call this.
///
/// Emits `("mm_sprd",)` with data `(maker_id, new_spread_bps)`.
pub fn update_spread(env: &Env, maker: &Address, maker_id: u64, new_spread_bps: u32) {
    maker.require_auth();

    let mut mm: MarketMaker = _get_maker_or_panic(env, maker_id);

    if mm.maker != *maker {
        panic_with_error!(env, MarketMakingError::Unauthorized);
    }
    if new_spread_bps == 0 || new_spread_bps > MAX_SPREAD_BPS {
        panic_with_error!(env, MarketMakingError::InvalidSpread);
    }

    mm.spread_bps = new_spread_bps;
    mm.updated_at = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&crate::DataKey::MarketMaking(MarketMakingKey::Maker(maker_id)), &mm);

    env.events()
        .publish((symbol_short!("mm_sprd"),), (maker_id, new_spread_bps));
}

/// Pauses or resumes a market maker. Only the maker owner may call this.
///
/// Emits `("mm_pause",)` or `("mm_resume",)` with data `(maker_id,)`.
pub fn set_maker_status(env: &Env, maker: &Address, maker_id: u64, active: bool) {
    maker.require_auth();

    let mut mm: MarketMaker = _get_maker_or_panic(env, maker_id);

    if mm.maker != *maker {
        panic_with_error!(env, MarketMakingError::Unauthorized);
    }
    if mm.status == MakerStatus::Closed {
        panic_with_error!(env, MarketMakingError::MakerNotActive);
    }

    mm.status = if active { MakerStatus::Active } else { MakerStatus::Paused };
    mm.updated_at = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&crate::DataKey::MarketMaking(MarketMakingKey::Maker(maker_id)), &mm);

    if active {
        env.events().publish((symbol_short!("mm_res"),), (maker_id,));
    } else {
        env.events().publish((symbol_short!("mm_pau"),), (maker_id,));
    }
}

/// Closes a market maker and withdraws all remaining inventory + fees to the owner.
///
/// Emits `("mm_close",)` with data `(maker_id, base_returned, quote_returned)`.
pub fn close_maker(env: &Env, maker: &Address, maker_id: u64) {
    maker.require_auth();

    let mut mm: MarketMaker = _get_maker_or_panic(env, maker_id);

    if mm.maker != *maker {
        panic_with_error!(env, MarketMakingError::Unauthorized);
    }
    if mm.status == MakerStatus::Closed {
        panic_with_error!(env, MarketMakingError::MakerNotActive);
    }

    let base_return = mm.base_inventory;
    let quote_return = mm.quote_inventory + mm.fees_earned;

    mm.base_inventory = 0;
    mm.quote_inventory = 0;
    mm.fees_earned = 0;
    mm.status = MakerStatus::Closed;
    mm.updated_at = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&crate::DataKey::MarketMaking(MarketMakingKey::Maker(maker_id)), &mm);

    _remove_active_maker(env, maker_id);

    if base_return > 0 {
        token::Client::new(env, &mm.base_token).transfer(
            &env.current_contract_address(),
            maker,
            &base_return,
        );
    }
    if quote_return > 0 {
        token::Client::new(env, &mm.quote_token).transfer(
            &env.current_contract_address(),
            maker,
            &quote_return,
        );
    }

    env.events()
        .publish((symbol_short!("mm_close"),), (maker_id, base_return, quote_return));
}

/// Returns a market maker by ID.
pub fn get_maker(env: &Env, maker_id: u64) -> Option<MarketMaker> {
    env.storage()
        .persistent()
        .get(&crate::DataKey::MarketMaking(MarketMakingKey::Maker(maker_id)))
}

/// Returns all maker IDs owned by `maker`.
pub fn get_owner_makers(env: &Env, maker: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&crate::DataKey::MarketMaking(MarketMakingKey::OwnerMakers(maker.clone())))
        .unwrap_or_else(|| Vec::new(env))
}

/// Returns all active maker IDs.
pub fn get_active_makers(env: &Env) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&crate::DataKey::MarketMaking(MarketMakingKey::ActiveMakers))
        .unwrap_or_else(|| Vec::new(env))
}

/// Returns maker IDs for a specific token pair.
pub fn get_pair_makers(env: &Env, base_token: &Address, quote_token: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&crate::DataKey::MarketMaking(MarketMakingKey::PairMakers(
            base_token.clone(),
            quote_token.clone(),
        )))
        .unwrap_or_else(|| Vec::new(env))
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn _get_maker_or_panic(env: &Env, maker_id: u64) -> MarketMaker {
    env.storage()
        .persistent()
        .get(&crate::DataKey::MarketMaking(MarketMakingKey::Maker(maker_id)))
        .unwrap_or_else(|| panic_with_error!(env, MarketMakingError::MakerNotFound))
}

fn _add_owner_maker(env: &Env, maker: &Address, maker_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&crate::DataKey::MarketMaking(MarketMakingKey::OwnerMakers(maker.clone())))
        .unwrap_or_else(|| Vec::new(env));
    if !ids.contains(&maker_id) {
        ids.push_back(maker_id);
        env.storage().persistent().set(
            &crate::DataKey::MarketMaking(MarketMakingKey::OwnerMakers(maker.clone())),
            &ids,
        );
    }
}

fn _add_active_maker(env: &Env, maker_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&crate::DataKey::MarketMaking(MarketMakingKey::ActiveMakers))
        .unwrap_or_else(|| Vec::new(env));
    if !ids.contains(&maker_id) {
        ids.push_back(maker_id);
        env.storage()
            .persistent()
            .set(&crate::DataKey::MarketMaking(MarketMakingKey::ActiveMakers), &ids);
    }
}

fn _remove_active_maker(env: &Env, maker_id: u64) {
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&crate::DataKey::MarketMaking(MarketMakingKey::ActiveMakers))
        .unwrap_or_else(|| Vec::new(env));
    let mut remaining = Vec::new(env);
    for id in ids.iter() {
        if id != maker_id {
            remaining.push_back(id);
        }
    }
    env.storage()
        .persistent()
        .set(&crate::DataKey::MarketMaking(MarketMakingKey::ActiveMakers), &remaining);
}

fn _add_pair_maker(env: &Env, base: &Address, quote: &Address, maker_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&crate::DataKey::MarketMaking(MarketMakingKey::PairMakers(
            base.clone(),
            quote.clone(),
        )))
        .unwrap_or_else(|| Vec::new(env));
    if !ids.contains(&maker_id) {
        ids.push_back(maker_id);
        env.storage().persistent().set(
            &crate::DataKey::MarketMaking(MarketMakingKey::PairMakers(base.clone(), quote.clone())),
            &ids,
        );
    }
}
