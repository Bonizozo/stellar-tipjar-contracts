//! Tip Derivatives Platform
//!
//! Unified module for complex tip-based financial instruments:
//! - **TipOption**: call/put options on tip token amounts
//! - **TipFuture**: forward contracts on tip delivery
//! - **TipSwap**: fixed-for-floating tip rate swaps
//!
//! Each instrument goes through a lifecycle:
//! `Open → Active → (Exercised | Settled | Expired | Liquidated)`
//!
//! Collateral is locked at open time and released at settlement.
//! Risk is managed via per-account position limits and margin health checks.

pub mod pricing;
pub mod risk;
pub mod settlement;

use soroban_sdk::{contracttype, token, Address, Env, Vec};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Basis-point denominator (10 000 = 100%).
pub const BPS: i128 = 10_000;

/// Fixed-point precision for prices (1_000_000 = 1.0).
pub const PRICE_PRECISION: i128 = 1_000_000;

/// Default initial margin requirement: 15% of notional.
pub const DEFAULT_INITIAL_MARGIN_BPS: i128 = 1_500;

/// Default maintenance margin: 7.5% of notional.
pub const DEFAULT_MAINTENANCE_MARGIN_BPS: i128 = 750;

/// Liquidation penalty: 3% of notional paid to liquidator.
pub const DEFAULT_LIQUIDATION_PENALTY_BPS: i128 = 300;

/// Maximum open positions per account.
pub const MAX_POSITIONS_PER_ACCOUNT: u32 = 50;

// ── Derivative types ─────────────────────────────────────────────────────────

/// The kind of derivative instrument.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum DerivativeKind {
    /// European call option: right to buy at strike.
    Call,
    /// European put option: right to sell at strike.
    Put,
    /// Cash-settled futures contract.
    Future,
    /// Fixed-for-floating tip rate swap.
    Swap,
}

/// Lifecycle state shared by all derivative types.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum DerivativeStatus {
    /// Waiting for a counterparty to match.
    Open,
    /// Both sides matched; contract is live.
    Active,
    /// Holder exercised the option before expiry.
    Exercised,
    /// Contract reached expiry and was cash-settled.
    Settled,
    /// Contract expired worthless (out-of-the-money).
    Expired,
    /// A party was liquidated due to insufficient margin.
    Liquidated,
    /// Cancelled before matching.
    Cancelled,
}

/// A unified derivative contract record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivativeContract {
    /// Unique contract ID.
    pub id: u64,
    /// Instrument type.
    pub kind: DerivativeKind,
    /// Current lifecycle status.
    pub status: DerivativeStatus,
    /// Initiating party (writer for options, long for futures/swaps).
    pub party_a: Address,
    /// Counterparty (holder for options, short for futures/swaps). None until matched.
    pub party_b: Option<Address>,
    /// Underlying tip token.
    pub token: Address,
    /// Notional size in token base units.
    pub notional: i128,
    /// Strike / contract price in token base units (PRICE_PRECISION scale).
    pub strike: i128,
    /// Premium paid by party_b to party_a at match time (options only).
    pub premium: i128,
    /// Collateral locked by party_a.
    pub collateral_a: i128,
    /// Collateral locked by party_b.
    pub collateral_b: i128,
    /// Last mark price (oracle feed, PRICE_PRECISION scale).
    pub mark_price: i128,
    /// Expiry / settlement timestamp (unix seconds).
    pub expires_at: u64,
    /// Creation timestamp.
    pub created_at: u64,
    /// Settlement price recorded at expiry.
    pub settlement_price: i128,
}

/// Per-account portfolio summary.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivativePortfolio {
    pub account: Address,
    /// Number of open/active contracts as party_a.
    pub initiated_count: u32,
    /// Number of open/active contracts as party_b.
    pub matched_count: u32,
    /// Total collateral locked across all contracts.
    pub total_collateral: i128,
    /// Cumulative realised P&L.
    pub realised_pnl: i128,
}

/// Global configuration for the derivatives module.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivativesConfig {
    pub initial_margin_bps: i128,
    pub maintenance_margin_bps: i128,
    pub liquidation_penalty_bps: i128,
    pub max_positions: u32,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

pub fn get_config(env: &Env) -> DerivativesConfig {
    env.storage()
        .persistent()
        .get(&DataKey::DerivativesConfig)
        .unwrap_or(DerivativesConfig {
            initial_margin_bps: DEFAULT_INITIAL_MARGIN_BPS,
            maintenance_margin_bps: DEFAULT_MAINTENANCE_MARGIN_BPS,
            liquidation_penalty_bps: DEFAULT_LIQUIDATION_PENALTY_BPS,
            max_positions: MAX_POSITIONS_PER_ACCOUNT,
        })
}

pub fn save_config(env: &Env, cfg: &DerivativesConfig) {
    env.storage()
        .persistent()
        .set(&DataKey::DerivativesConfig, cfg);
}

pub fn get_contract(env: &Env, id: u64) -> Option<DerivativeContract> {
    env.storage()
        .persistent()
        .get(&DataKey::Derivative(id))
}

pub fn save_contract(env: &Env, dc: &DerivativeContract) {
    env.storage()
        .persistent()
        .set(&DataKey::Derivative(dc.id), dc);
}

fn next_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::DerivativeCounter)
        .unwrap_or(0u64)
        + 1;
    env.storage()
        .persistent()
        .set(&DataKey::DerivativeCounter, &id);
    id
}

pub fn get_portfolio(env: &Env, account: &Address) -> DerivativePortfolio {
    env.storage()
        .persistent()
        .get(&DataKey::DerivativePortfolio(account.clone()))
        .unwrap_or(DerivativePortfolio {
            account: account.clone(),
            initiated_count: 0,
            matched_count: 0,
            total_collateral: 0,
            realised_pnl: 0,
        })
}

pub fn save_portfolio(env: &Env, p: &DerivativePortfolio) {
    env.storage()
        .persistent()
        .set(&DataKey::DerivativePortfolio(p.account.clone()), p);
}

pub fn get_account_contracts(env: &Env, account: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::DerivativeAccountContracts(account.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

fn add_account_contract(env: &Env, account: &Address, id: u64) {
    let mut list = get_account_contracts(env, account);
    if !list.contains(&id) {
        list.push_back(id);
        env.storage()
            .persistent()
            .set(&DataKey::DerivativeAccountContracts(account.clone()), &list);
    }
}

pub fn get_active_ids(env: &Env) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::DerivativeActiveList)
        .unwrap_or_else(|| Vec::new(env))
}

fn add_active(env: &Env, id: u64) {
    let mut list = get_active_ids(env);
    if !list.contains(&id) {
        list.push_back(id);
        env.storage()
            .persistent()
            .set(&DataKey::DerivativeActiveList, &list);
    }
}

pub fn remove_active(env: &Env, id: u64) {
    let list = get_active_ids(env);
    let mut updated: Vec<u64> = Vec::new(env);
    for i in 0..list.len() {
        let v = list.get(i).unwrap();
        if v != id {
            updated.push_back(v);
        }
    }
    env.storage()
        .persistent()
        .set(&DataKey::DerivativeActiveList, &updated);
}

// ── Core operations ──────────────────────────────────────────────────────────

/// Open a new derivative contract. The initiating party (`party_a`) posts
/// initial collateral. Returns the new contract ID.
///
/// For options, `premium` is the amount party_b will pay at match time.
/// For futures/swaps, `premium` should be 0.
pub fn open(
    env: &Env,
    party_a: &Address,
    kind: DerivativeKind,
    token: &Address,
    notional: i128,
    strike: i128,
    premium: i128,
    expires_at: u64,
) -> u64 {
    assert!(notional > 0, "notional must be positive");
    assert!(strike > 0, "strike must be positive");
    assert!(expires_at > env.ledger().timestamp(), "expiry must be in the future");

    let cfg = get_config(env);
    risk::check_position_limit(env, party_a, &cfg);

    let collateral_a = required_collateral(kind, notional, strike, cfg.initial_margin_bps);

    let token_client = token::Client::new(env, token);
    token_client.transfer(party_a, &env.current_contract_address(), &collateral_a);

    let id = next_id(env);
    let now = env.ledger().timestamp();

    let dc = DerivativeContract {
        id,
        kind,
        status: DerivativeStatus::Open,
        party_a: party_a.clone(),
        party_b: None,
        token: token.clone(),
        notional,
        strike,
        premium,
        collateral_a,
        collateral_b: 0,
        mark_price: strike,
        expires_at,
        created_at: now,
        settlement_price: 0,
    };

    save_contract(env, &dc);
    add_active(env, id);
    add_account_contract(env, party_a, id);

    let mut portfolio = get_portfolio(env, party_a);
    portfolio.initiated_count += 1;
    portfolio.total_collateral += collateral_a;
    save_portfolio(env, &portfolio);

    id
}

/// Match the counterparty (`party_b`) to an open contract.
/// For options, party_b pays the premium to party_a and posts their collateral.
/// For futures/swaps, party_b posts initial margin.
pub fn match_contract(env: &Env, party_b: &Address, id: u64) {
    let mut dc = get_contract(env, id).expect("contract not found");
    assert!(dc.status == DerivativeStatus::Open, "contract not open");
    assert!(dc.party_b.is_none(), "already matched");

    let cfg = get_config(env);
    risk::check_position_limit(env, party_b, &cfg);

    let token_client = token::Client::new(env, &dc.token);

    // For options: party_b pays premium to party_a and posts collateral
    // For futures/swaps: party_b posts initial margin
    let collateral_b = match dc.kind {
        DerivativeKind::Call | DerivativeKind::Put => {
            if dc.premium > 0 {
                token_client.transfer(party_b, &dc.party_a, &dc.premium);
            }
            // Buyer posts a smaller margin (premium already paid)
            required_collateral(dc.kind, dc.notional, dc.strike, cfg.initial_margin_bps / 2)
        }
        DerivativeKind::Future | DerivativeKind::Swap => {
            required_collateral(dc.kind, dc.notional, dc.strike, cfg.initial_margin_bps)
        }
    };

    if collateral_b > 0 {
        token_client.transfer(party_b, &env.current_contract_address(), &collateral_b);
    }

    dc.party_b = Some(party_b.clone());
    dc.collateral_b = collateral_b;
    dc.status = DerivativeStatus::Active;
    save_contract(env, &dc);
    add_account_contract(env, party_b, id);

    let mut portfolio = get_portfolio(env, party_b);
    portfolio.matched_count += 1;
    portfolio.total_collateral += collateral_b;
    save_portfolio(env, &portfolio);
}

/// Update the mark price for an active contract (called by oracle/admin).
pub fn update_mark_price(env: &Env, id: u64, new_price: i128) {
    let mut dc = get_contract(env, id).expect("contract not found");
    assert!(dc.status == DerivativeStatus::Active, "contract not active");
    assert!(new_price > 0, "price must be positive");
    dc.mark_price = new_price;
    save_contract(env, &dc);
}

/// Cancel an unmatched (Open) contract and refund party_a's collateral.
pub fn cancel(env: &Env, caller: &Address, id: u64) {
    let mut dc = get_contract(env, id).expect("contract not found");
    assert!(dc.status == DerivativeStatus::Open, "can only cancel open contracts");
    assert!(dc.party_a == *caller, "not the contract initiator");

    let token_client = token::Client::new(env, &dc.token);
    if dc.collateral_a > 0 {
        token_client.transfer(&env.current_contract_address(), caller, &dc.collateral_a);
    }

    let mut portfolio = get_portfolio(env, caller);
    portfolio.initiated_count = portfolio.initiated_count.saturating_sub(1);
    portfolio.total_collateral = portfolio.total_collateral.saturating_sub(dc.collateral_a);
    save_portfolio(env, &portfolio);

    dc.status = DerivativeStatus::Cancelled;
    save_contract(env, &dc);
    remove_active(env, id);
}

/// Liquidate an under-margined position. The liquidator receives the penalty.
/// Returns the penalty amount paid to the liquidator.
pub fn liquidate(env: &Env, liquidator: &Address, id: u64) -> i128 {
    let mut dc = get_contract(env, id).expect("contract not found");
    assert!(dc.status == DerivativeStatus::Active, "contract not active");

    let cfg = get_config(env);
    let (under_a, under_b) = risk::check_margin_health(&dc, &cfg);
    assert!(under_a || under_b, "no under-margined position");

    let token_client = token::Client::new(env, &dc.token);
    let notional_value = dc.notional * dc.mark_price / PRICE_PRECISION;
    let penalty = notional_value * cfg.liquidation_penalty_bps / BPS;

    if under_a {
        let penalty_actual = penalty.min(dc.collateral_a);
        if penalty_actual > 0 {
            token_client.transfer(&env.current_contract_address(), liquidator, &penalty_actual);
        }
        let remaining = dc.collateral_a - penalty_actual;
        if remaining > 0 {
            token_client.transfer(&env.current_contract_address(), &dc.party_a, &remaining);
        }
        // Return party_b's collateral
        if let Some(ref pb) = dc.party_b.clone() {
            if dc.collateral_b > 0 {
                token_client.transfer(&env.current_contract_address(), pb, &dc.collateral_b);
            }
            let mut p = get_portfolio(env, pb);
            p.matched_count = p.matched_count.saturating_sub(1);
            p.total_collateral = p.total_collateral.saturating_sub(dc.collateral_b);
            save_portfolio(env, &p);
        }
        let mut p = get_portfolio(env, &dc.party_a);
        p.initiated_count = p.initiated_count.saturating_sub(1);
        p.total_collateral = p.total_collateral.saturating_sub(dc.collateral_a);
        save_portfolio(env, &p);

        dc.status = DerivativeStatus::Liquidated;
        save_contract(env, &dc);
        remove_active(env, id);
        return penalty_actual;
    }

    // under_b
    let party_b = dc.party_b.clone().expect("no party_b");
    let penalty_actual = penalty.min(dc.collateral_b);
    if penalty_actual > 0 {
        token_client.transfer(&env.current_contract_address(), liquidator, &penalty_actual);
    }
    let remaining = dc.collateral_b - penalty_actual;
    if remaining > 0 {
        token_client.transfer(&env.current_contract_address(), &party_b, &remaining);
    }
    // Return party_a's collateral
    if dc.collateral_a > 0 {
        token_client.transfer(&env.current_contract_address(), &dc.party_a, &dc.collateral_a);
    }
    let mut pa = get_portfolio(env, &dc.party_a);
    pa.initiated_count = pa.initiated_count.saturating_sub(1);
    pa.total_collateral = pa.total_collateral.saturating_sub(dc.collateral_a);
    save_portfolio(env, &pa);

    let mut pb = get_portfolio(env, &party_b);
    pb.matched_count = pb.matched_count.saturating_sub(1);
    pb.total_collateral = pb.total_collateral.saturating_sub(dc.collateral_b);
    save_portfolio(env, &pb);

    dc.status = DerivativeStatus::Liquidated;
    save_contract(env, &dc);
    remove_active(env, id);

    penalty_actual
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Calculate required collateral for a given instrument and margin rate.
pub fn required_collateral(
    kind: DerivativeKind,
    notional: i128,
    strike: i128,
    margin_bps: i128,
) -> i128 {
    let notional_value = match kind {
        DerivativeKind::Call => notional,
        DerivativeKind::Put => notional * strike / PRICE_PRECISION,
        DerivativeKind::Future | DerivativeKind::Swap => notional * strike / PRICE_PRECISION,
    };
    (notional_value * margin_bps / BPS).max(1)
}
