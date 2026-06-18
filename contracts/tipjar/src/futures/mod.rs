//! Tip Futures Contracts
//!
//! This module implements futures contracts for tip commitments. A futures
//! contract is an agreement to deliver a specified tip amount to a creator
//! at a future settlement date, at a price agreed today.
//!
//! # Mechanics
//! - **Long** (buyer): commits to receive the tip delivery; profits if the
//!   creator's tip rate rises above the contract price.
//! - **Short** (seller / creator): commits to deliver the tip; profits if
//!   the actual tip rate stays below the contract price.
//! - Both sides post **initial margin** (a % of notional) when opening.
//! - **Mark-to-market** is performed via an oracle price feed; unrealised
//!   P&L is tracked and margin is checked against a **maintenance margin**.
//! - Positions below maintenance margin are **liquidatable** by any caller.
//! - At **settlement** the contract is cash-settled against the final oracle
//!   price; net P&L is transferred between the two sides.

pub mod margin;
pub mod settlement;

use soroban_sdk::{contracttype, token, Address, Env, Vec};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Basis-point denominator (10 000 = 100 %).
pub const BPS_DENOM: i128 = 10_000;

/// Default initial margin requirement: 10 % of notional.
pub const DEFAULT_INITIAL_MARGIN_BPS: u32 = 1_000;

/// Default maintenance margin: 5 % of notional.
pub const DEFAULT_MAINTENANCE_MARGIN_BPS: u32 = 500;

/// Liquidation penalty paid to the liquidator: 2 % of notional.
pub const DEFAULT_LIQUIDATION_PENALTY_BPS: u32 = 200;

/// Minimum contract size in token base units.
pub const MIN_CONTRACT_SIZE: i128 = 1_000;

/// Precision multiplier for P&L calculations (1_000_000 = 1.0).
pub const PRICE_PRECISION: i128 = 1_000_000;

// ── Data types ───────────────────────────────────────────────────────────────

/// Direction of a futures position.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum Side {
    /// Buyer — profits when price rises.
    Long,
    /// Seller — profits when price falls.
    Short,
}

/// Lifecycle state of a futures contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum FuturesStatus {
    /// Both sides have posted margin; contract is live.
    Active,
    /// Settlement date reached; awaiting final settlement call.
    PendingSettlement,
    /// Contract has been cash-settled.
    Settled,
    /// One side was liquidated before settlement.
    Liquidated,
    /// Contract was cancelled before a counterparty matched.
    Cancelled,
}

/// A futures contract between a long and a short party.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FuturesContract {
    /// Unique contract ID.
    pub contract_id: u64,
    /// Long side (buyer).
    pub long_party: Address,
    /// Short side (seller / creator). `None` until matched.
    pub short_party: Option<Address>,
    /// Token used for margin and settlement.
    pub token: Address,
    /// Agreed contract price (tip amount) in token base units.
    pub contract_price: i128,
    /// Notional size: number of token units the contract covers.
    pub size: i128,
    /// Unix timestamp of the settlement date.
    pub settles_at: u64,
    /// Current contract status.
    pub status: FuturesStatus,
    /// Margin posted by the long party.
    pub long_margin: i128,
    /// Margin posted by the short party.
    pub short_margin: i128,
    /// Last mark price used for P&L (set by oracle updates).
    pub mark_price: i128,
    /// Unrealised P&L for the long side (can be negative).
    pub long_unrealised_pnl: i128,
    /// Initial margin requirement in basis points.
    pub initial_margin_bps: u32,
    /// Maintenance margin requirement in basis points.
    pub maintenance_margin_bps: u32,
    /// Liquidation penalty in basis points.
    pub liquidation_penalty_bps: u32,
    /// Creation timestamp.
    pub created_at: u64,
}

/// Aggregated position summary for a trader across all futures.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FuturesPosition {
    pub trader: Address,
    /// Number of active long contracts.
    pub long_count: u32,
    /// Number of active short contracts.
    pub short_count: u32,
    /// Total margin locked across all contracts.
    pub total_margin: i128,
    /// Cumulative realised P&L.
    pub realised_pnl: i128,
}

/// Global configuration for the futures module.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FuturesConfig {
    pub initial_margin_bps: u32,
    pub maintenance_margin_bps: u32,
    pub liquidation_penalty_bps: u32,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

pub fn get_contract(env: &Env, contract_id: u64) -> Option<FuturesContract> {
    env.storage()
        .persistent()
        .get(&DataKey::FuturesContract(contract_id))
}

pub fn save_contract(env: &Env, fc: &FuturesContract) {
    env.storage()
        .persistent()
        .set(&DataKey::FuturesContract(fc.contract_id), fc);
}

pub fn get_counter(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::FuturesCounter)
        .unwrap_or(0u64)
}

pub fn next_id(env: &Env) -> u64 {
    let id = get_counter(env) + 1;
    env.storage()
        .persistent()
        .set(&DataKey::FuturesCounter, &id);
    id
}

pub fn get_position(env: &Env, trader: &Address) -> FuturesPosition {
    env.storage()
        .persistent()
        .get(&DataKey::FuturesPosition(trader.clone()))
        .unwrap_or(FuturesPosition {
            trader: trader.clone(),
            long_count: 0,
            short_count: 0,
            total_margin: 0,
            realised_pnl: 0,
        })
}

pub fn save_position(env: &Env, pos: &FuturesPosition) {
    env.storage()
        .persistent()
        .set(&DataKey::FuturesPosition(pos.trader.clone()), pos);
}

pub fn get_trader_contracts(env: &Env, trader: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::FuturesTraderContracts(trader.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn add_trader_contract(env: &Env, trader: &Address, contract_id: u64) {
    let mut list = get_trader_contracts(env, trader);
    if !list.contains(contract_id) {
        list.push_back(contract_id);
        env.storage()
            .persistent()
            .set(&DataKey::FuturesTraderContracts(trader.clone()), &list);
    }
}

pub fn get_active_contracts(env: &Env) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::FuturesActiveContracts)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn add_active_contract(env: &Env, contract_id: u64) {
    let mut list = get_active_contracts(env);
    if !list.contains(contract_id) {
        list.push_back(contract_id);
        env.storage()
            .persistent()
            .set(&DataKey::FuturesActiveContracts, &list);
    }
}

pub fn remove_active_contract(env: &Env, contract_id: u64) {
    let list = get_active_contracts(env);
    let mut updated: Vec<u64> = Vec::new(env);
    for i in 0..list.len() {
        let id = list.get(i).unwrap();
        if id != contract_id {
            updated.push_back(id);
        }
    }
    env.storage()
        .persistent()
        .set(&DataKey::FuturesActiveContracts, &updated);
}

pub fn get_config(env: &Env) -> FuturesConfig {
    env.storage()
        .persistent()
        .get(&DataKey::FuturesConfig)
        .unwrap_or(FuturesConfig {
            initial_margin_bps: DEFAULT_INITIAL_MARGIN_BPS,
            maintenance_margin_bps: DEFAULT_MAINTENANCE_MARGIN_BPS,
            liquidation_penalty_bps: DEFAULT_LIQUIDATION_PENALTY_BPS,
        })
}

pub fn save_config(env: &Env, cfg: &FuturesConfig) {
    env.storage().persistent().set(&DataKey::FuturesConfig, cfg);
}

// ── Core logic ───────────────────────────────────────────────────────────────

/// Open a new futures contract (long side). The long party posts initial margin.
/// Returns the new contract ID.
pub fn open_long(
    env: &Env,
    long_party: &Address,
    token: &Address,
    contract_price: i128,
    size: i128,
    settles_at: u64,
) -> u64 {
    let cfg = get_config(env);
    let required_margin =
        margin::required_initial_margin(size, contract_price, cfg.initial_margin_bps);

    // Transfer margin from long party to contract
    let token_client = token::Client::new(env, token);
    token_client.transfer(
        long_party,
        &env.current_contract_address(),
        &required_margin,
    );

    let contract_id = next_id(env);
    let now = env.ledger().timestamp();

    let fc = FuturesContract {
        contract_id,
        long_party: long_party.clone(),
        short_party: None,
        token: token.clone(),
        contract_price,
        size,
        settles_at,
        status: FuturesStatus::Active,
        long_margin: required_margin,
        short_margin: 0,
        mark_price: contract_price,
        long_unrealised_pnl: 0,
        initial_margin_bps: cfg.initial_margin_bps,
        maintenance_margin_bps: cfg.maintenance_margin_bps,
        liquidation_penalty_bps: cfg.liquidation_penalty_bps,
        created_at: now,
    };

    save_contract(env, &fc);
    add_active_contract(env, contract_id);
    add_trader_contract(env, long_party, contract_id);

    let mut pos = get_position(env, long_party);
    pos.long_count += 1;
    pos.total_margin += required_margin;
    save_position(env, &pos);

    contract_id
}

/// Match the short side of an existing unmatched futures contract.
/// The short party posts initial margin.
pub fn match_short(env: &Env, short_party: &Address, contract_id: u64) {
    let mut fc = get_contract(env, contract_id).expect("contract not found");
    assert!(fc.status == FuturesStatus::Active, "contract not active");
    assert!(fc.short_party.is_none(), "contract already matched");

    let required_margin =
        margin::required_initial_margin(fc.size, fc.contract_price, fc.initial_margin_bps);

    let token_client = token::Client::new(env, &fc.token);
    token_client.transfer(
        short_party,
        &env.current_contract_address(),
        &required_margin,
    );

    fc.short_party = Some(short_party.clone());
    fc.short_margin = required_margin;
    save_contract(env, &fc);

    add_trader_contract(env, short_party, contract_id);

    let mut pos = get_position(env, short_party);
    pos.short_count += 1;
    pos.total_margin += required_margin;
    save_position(env, &pos);
}

/// Update the mark price and recalculate unrealised P&L.
/// Callable by the designated oracle / admin.
pub fn update_mark_price(env: &Env, contract_id: u64, new_price: i128) {
    let mut fc = get_contract(env, contract_id).expect("contract not found");
    assert!(fc.status == FuturesStatus::Active, "contract not active");

    // Long P&L = (mark_price - contract_price) * size / PRICE_PRECISION
    fc.long_unrealised_pnl = (new_price - fc.contract_price) * fc.size / PRICE_PRECISION;
    fc.mark_price = new_price;
    save_contract(env, &fc);
}

/// Add margin to a position (long or short) to avoid liquidation.
pub fn add_margin(env: &Env, trader: &Address, contract_id: u64, side: Side, amount: i128) {
    let mut fc = get_contract(env, contract_id).expect("contract not found");
    assert!(fc.status == FuturesStatus::Active, "contract not active");

    let token_client = token::Client::new(env, &fc.token);
    token_client.transfer(trader, &env.current_contract_address(), &amount);

    match side {
        Side::Long => {
            assert!(fc.long_party == *trader, "not the long party");
            fc.long_margin += amount;
        }
        Side::Short => {
            let short = fc.short_party.as_ref().expect("no short party");
            assert!(short == trader, "not the short party");
            fc.short_margin += amount;
        }
    }
    save_contract(env, &fc);

    let mut pos = get_position(env, trader);
    pos.total_margin += amount;
    save_position(env, &pos);
}

/// Liquidate an under-margined position.
/// The liquidator receives the liquidation penalty from the liquidated party's margin.
/// Returns the penalty amount paid to the liquidator.
pub fn liquidate(env: &Env, liquidator: &Address, contract_id: u64) -> i128 {
    let mut fc = get_contract(env, contract_id).expect("contract not found");
    assert!(fc.status == FuturesStatus::Active, "contract not active");
    assert!(fc.short_party.is_some(), "contract not matched");

    let (liquidated_side, liquidated_party, liquidated_margin) =
        margin::find_liquidatable_side(&fc);

    let liquidated_party = liquidated_party.expect("no liquidatable side");
    assert!(liquidated_margin >= 0, "no margin to liquidate");

    let penalty = fc.size * fc.contract_price / PRICE_PRECISION
        * fc.liquidation_penalty_bps as i128
        / BPS_DENOM;

    let token_client = token::Client::new(env, &fc.token);

    // Pay penalty to liquidator from the liquidated party's margin
    let penalty_actual = penalty.min(liquidated_margin);
    if penalty_actual > 0 {
        token_client.transfer(&env.current_contract_address(), liquidator, &penalty_actual);
    }

    // Return remaining margin to the liquidated party
    let remaining = liquidated_margin - penalty_actual;
    if remaining > 0 {
        token_client.transfer(
            &env.current_contract_address(),
            &liquidated_party,
            &remaining,
        );
    }

    // Return the other side's margin to them
    match liquidated_side {
        Side::Long => {
            if fc.short_margin > 0 {
                let short = fc.short_party.clone().unwrap();
                token_client.transfer(&env.current_contract_address(), &short, &fc.short_margin);
            }
            let mut pos = get_position(env, &fc.long_party);
            pos.long_count = pos.long_count.saturating_sub(1);
            pos.total_margin = pos.total_margin.saturating_sub(fc.long_margin);
            save_position(env, &pos);
        }
        Side::Short => {
            if fc.long_margin > 0 {
                token_client.transfer(
                    &env.current_contract_address(),
                    &fc.long_party,
                    &fc.long_margin,
                );
            }
            let short = fc.short_party.clone().unwrap();
            let mut pos = get_position(env, &short);
            pos.short_count = pos.short_count.saturating_sub(1);
            pos.total_margin = pos.total_margin.saturating_sub(fc.short_margin);
            save_position(env, &pos);
        }
    }

    fc.status = FuturesStatus::Liquidated;
    save_contract(env, &fc);
    remove_active_contract(env, contract_id);

    penalty_actual
}

/// Cancel an unmatched contract and refund the long party's margin.
pub fn cancel_contract(env: &Env, caller: &Address, contract_id: u64) {
    let mut fc = get_contract(env, contract_id).expect("contract not found");
    assert!(fc.status == FuturesStatus::Active, "contract not active");
    assert!(fc.short_party.is_none(), "contract already matched");
    assert!(fc.long_party == *caller, "not the contract owner");

    let token_client = token::Client::new(env, &fc.token);
    if fc.long_margin > 0 {
        token_client.transfer(&env.current_contract_address(), caller, &fc.long_margin);
    }

    let mut pos = get_position(env, caller);
    pos.long_count = pos.long_count.saturating_sub(1);
    pos.total_margin = pos.total_margin.saturating_sub(fc.long_margin);
    save_position(env, &pos);

    fc.status = FuturesStatus::Cancelled;
    save_contract(env, &fc);
    remove_active_contract(env, contract_id);
}
