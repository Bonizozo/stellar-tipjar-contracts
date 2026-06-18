//! Tip Prediction Markets
//!
//! This module provides prediction market functionality for betting on creator
//! success metrics (e.g. total tips received, subscriber count milestones).
//!
//! # Overview
//! - Markets are created by admins or creators around a measurable outcome.
//! - Participants place bets on one of two outcomes (Yes / No).
//! - Odds are calculated using a parimutuel model (see `odds` submodule).
//! - A designated resolver settles the market once the outcome is known.
//! - Winners receive a proportional share of the total pool minus a platform fee.

pub mod odds;
pub mod settlement;

use soroban_sdk::{contracttype, token, Address, Env, String, Vec};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Basis-point denominator (10 000 = 100 %).
pub const BPS_DENOM: u32 = 10_000;

/// Default platform fee charged on winnings (2 %).
pub const DEFAULT_FEE_BPS: u32 = 200;

/// Minimum bet amount (in token base units).
pub const MIN_BET_AMOUNT: i128 = 1_000;

/// Precision multiplier used for odds calculations.
pub const ODDS_PRECISION: i128 = 1_000_000;

// ── Data types ───────────────────────────────────────────────────────────────

/// The two sides of a prediction market.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum Outcome {
    None,
    Yes,
    No,
}

/// Lifecycle state of a market.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum MarketStatus {
    /// Accepting bets.
    Open,
    /// Betting window closed; awaiting resolution.
    Closed,
    /// Outcome has been determined; winnings can be claimed.
    Resolved,
    /// Market was cancelled; all bets are refundable.
    Cancelled,
}

/// A prediction market for a creator success metric.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredictionMarket {
    /// Unique market identifier.
    pub market_id: u64,
    /// Creator whose metric is being predicted.
    pub creator: Address,
    /// Address authorised to resolve the market.
    pub resolver: Address,
    /// Human-readable description of the prediction question.
    pub question: String,
    /// Token used for betting.
    pub token: Address,
    /// Total amount bet on the Yes outcome.
    pub yes_pool: i128,
    /// Total amount bet on the No outcome.
    pub no_pool: i128,
    /// Unix timestamp after which no new bets are accepted.
    pub closes_at: u64,
    /// Unix timestamp after which the resolver must have settled.
    pub resolves_at: u64,
    /// Current lifecycle status.
    pub status: MarketStatus,
    /// Winning outcome (set on resolution).
    pub winning_outcome: Outcome,
    /// Platform fee in basis points.
    pub fee_bps: u32,
    /// Creation timestamp.
    pub created_at: u64,
}

/// Aggregated position for a bettor in a market.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BettorPosition {
    pub bettor: Address,
    pub market_id: u64,
    /// Total amount bet on Yes.
    pub yes_amount: i128,
    /// Total amount bet on No.
    pub no_amount: i128,
    /// Whether the position has been settled (claimed or refunded).
    pub settled: bool,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

pub fn get_market(env: &Env, market_id: u64) -> Option<PredictionMarket> {
    env.storage()
        .persistent()
        .get(&DataKey::PredMarket(market_id))
}

pub fn save_market(env: &Env, market: &PredictionMarket) {
    env.storage()
        .persistent()
        .set(&DataKey::PredMarket(market.market_id), market);
}

pub fn get_market_counter(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::PredMarketCounter)
        .unwrap_or(0u64)
}

pub fn increment_market_counter(env: &Env) -> u64 {
    let id = get_market_counter(env) + 1;
    env.storage()
        .persistent()
        .set(&DataKey::PredMarketCounter, &id);
    id
}

pub fn get_bettor_position(env: &Env, market_id: u64, bettor: &Address) -> BettorPosition {
    env.storage()
        .persistent()
        .get(&DataKey::PredBettorPosition(market_id, bettor.clone()))
        .unwrap_or(BettorPosition {
            bettor: bettor.clone(),
            market_id,
            yes_amount: 0,
            no_amount: 0,
            settled: false,
        })
}

pub fn save_bettor_position(env: &Env, position: &BettorPosition) {
    env.storage().persistent().set(
        &DataKey::PredBettorPosition(position.market_id, position.bettor.clone()),
        position,
    );
}

pub fn get_active_markets(env: &Env) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::PredActiveMarkets)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn add_active_market(env: &Env, market_id: u64) {
    let mut list = get_active_markets(env);
    if !list.contains(market_id) {
        list.push_back(market_id);
        env.storage()
            .persistent()
            .set(&DataKey::PredActiveMarkets, &list);
    }
}

pub fn remove_active_market(env: &Env, market_id: u64) {
    let list = get_active_markets(env);
    let mut updated: Vec<u64> = Vec::new(env);
    for i in 0..list.len() {
        let id = list.get(i).unwrap();
        if id != market_id {
            updated.push_back(id);
        }
    }
    env.storage()
        .persistent()
        .set(&DataKey::PredActiveMarkets, &updated);
}

pub fn get_creator_markets(env: &Env, creator: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::PredCreatorMarkets(creator.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn add_creator_market(env: &Env, creator: &Address, market_id: u64) {
    let mut list = get_creator_markets(env, creator);
    if !list.contains(market_id) {
        list.push_back(market_id);
        env.storage()
            .persistent()
            .set(&DataKey::PredCreatorMarkets(creator.clone()), &list);
    }
}

pub fn get_bettor_markets(env: &Env, bettor: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::PredBettorMarkets(bettor.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn add_bettor_market(env: &Env, bettor: &Address, market_id: u64) {
    let mut list = get_bettor_markets(env, bettor);
    if !list.contains(market_id) {
        list.push_back(market_id);
        env.storage()
            .persistent()
            .set(&DataKey::PredBettorMarkets(bettor.clone()), &list);
    }
}

pub fn get_fee_bps(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::PredMarketFeeBps)
        .unwrap_or(DEFAULT_FEE_BPS)
}

pub fn set_fee_bps(env: &Env, fee_bps: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::PredMarketFeeBps, &fee_bps);
}

// ── Core logic ───────────────────────────────────────────────────────────────

/// Create a new prediction market. Returns the new market ID.
pub fn create_market(
    env: &Env,
    creator: &Address,
    resolver: &Address,
    question: String,
    token: &Address,
    closes_at: u64,
    resolves_at: u64,
) -> u64 {
    let now = env.ledger().timestamp();
    let market_id = increment_market_counter(env);
    let fee_bps = get_fee_bps(env);

    let market = PredictionMarket {
        market_id,
        creator: creator.clone(),
        resolver: resolver.clone(),
        question,
        token: token.clone(),
        yes_pool: 0,
        no_pool: 0,
        closes_at,
        resolves_at,
        status: MarketStatus::Open,
        winning_outcome: Outcome::None,
        fee_bps,
        created_at: now,
    };

    save_market(env, &market);
    add_active_market(env, market_id);
    add_creator_market(env, creator, market_id);

    market_id
}

/// Place a bet on a market outcome. Transfers tokens from bettor to contract.
pub fn place_bet(env: &Env, bettor: &Address, market_id: u64, outcome: Outcome, amount: i128) {
    let mut market = get_market(env, market_id).expect("market not found");

    let now = env.ledger().timestamp();
    assert!(market.status == MarketStatus::Open, "market not open");
    assert!(now < market.closes_at, "betting window closed");
    assert!(amount >= MIN_BET_AMOUNT, "bet below minimum");

    // Transfer tokens from bettor to contract
    let token_client = token::Client::new(env, &market.token);
    token_client.transfer(bettor, &env.current_contract_address(), &amount);

    // Update pool totals
    match outcome {
        Outcome::None => panic!("invalid outcome"),
        Outcome::Yes => market.yes_pool += amount,
        Outcome::No => market.no_pool += amount,
    }
    save_market(env, &market);

    // Update bettor position
    let mut position = get_bettor_position(env, market_id, bettor);
    match outcome {
        Outcome::None => panic!("invalid outcome"),
        Outcome::Yes => position.yes_amount += amount,
        Outcome::No => position.no_amount += amount,
    }
    save_bettor_position(env, &position);
    add_bettor_market(env, bettor, market_id);
}

/// Close a market's betting window (callable by resolver or after closes_at).
pub fn close_market(env: &Env, caller: &Address, market_id: u64) {
    let mut market = get_market(env, market_id).expect("market not found");
    assert!(market.status == MarketStatus::Open, "market not open");
    assert!(
        *caller == market.resolver || env.ledger().timestamp() >= market.closes_at,
        "not authorised to close"
    );
    market.status = MarketStatus::Closed;
    save_market(env, &market);
}

/// Resolve a market with the winning outcome. Only the resolver may call this.
pub fn resolve_market(env: &Env, resolver: &Address, market_id: u64, winning: Outcome) {
    let mut market = get_market(env, market_id).expect("market not found");
    assert!(
        market.status == MarketStatus::Open || market.status == MarketStatus::Closed,
        "market already resolved or cancelled"
    );
    assert!(*resolver == market.resolver, "not the resolver");

    market.status = MarketStatus::Resolved;
    market.winning_outcome = winning;
    save_market(env, &market);
    remove_active_market(env, market_id);
}

/// Cancel a market and allow refunds. Admin or resolver only.
pub fn cancel_market(env: &Env, caller: &Address, market_id: u64, admin: &Address) {
    let mut market = get_market(env, market_id).expect("market not found");
    assert!(
        market.status == MarketStatus::Open || market.status == MarketStatus::Closed,
        "cannot cancel resolved market"
    );
    assert!(
        *caller == market.resolver || *caller == *admin,
        "not authorised"
    );
    market.status = MarketStatus::Cancelled;
    save_market(env, &market);
    remove_active_market(env, market_id);
}

/// Claim winnings or refund for a settled/cancelled market.
/// Returns the payout amount.
pub fn claim_winnings(env: &Env, bettor: &Address, market_id: u64) -> i128 {
    let market = get_market(env, market_id).expect("market not found");
    assert!(
        market.status == MarketStatus::Resolved || market.status == MarketStatus::Cancelled,
        "market not settled"
    );

    let mut position = get_bettor_position(env, market_id, bettor);
    assert!(!position.settled, "already claimed");

    let payout = if market.status == MarketStatus::Cancelled {
        // Full refund of all bets
        position.yes_amount + position.no_amount
    } else {
        settlement::calculate_payout(env, &market, &position)
    };

    if payout > 0 {
        let token_client = token::Client::new(env, &market.token);
        token_client.transfer(&env.current_contract_address(), bettor, &payout);
    }

    position.settled = true;
    save_bettor_position(env, &position);

    payout
}
