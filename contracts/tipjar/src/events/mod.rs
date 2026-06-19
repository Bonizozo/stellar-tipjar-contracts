//! Advanced Event System with Filtering and Indexing
//!
//! This module provides enhanced event tracking, filtering, and querying capabilities
//! for the TipJar contract.

pub mod filters;
pub mod indexing;

use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Vec};

/// Structured tip event for reliable indexing
/// Emitted when a tip is sent to a creator
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipEvent {
    /// Event version for schema evolution
    pub version: u32,
    /// Unique event ID for tracking
    pub event_id: u64,
    /// Ledger timestamp when tip was sent
    pub timestamp: u64,
    /// Address of the tipper
    pub sender: Address,
    /// Address of the creator receiving the tip
    pub creator: Address,
    /// Tip amount in smallest units
    pub amount: i128,
    /// Token address
    pub token: Address,
}

/// Structured withdraw event for reliable indexing
/// Emitted when a creator withdraws their escrowed tips
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawEvent {
    /// Event version for schema evolution
    pub version: u32,
    /// Address of the creator withdrawing
    pub creator: Address,
    /// Amount withdrawn in smallest units
    pub amount: i128,
    /// Token address
    pub token: Address,
    /// Ledger timestamp when withdrawal occurred
    pub timestamp: u64,
}

/// Current event version for schema evolution
pub const EVENT_VERSION: u32 = 1;

/// Emit a structured tip event
/// 
/// # Event Schema
/// - **Topic**: `tip` (symbol)
/// - **Data**: TipEvent struct with named fields (sender, creator, amount, token, timestamp)
/// - **Filterable By**: creator (topics), sender and creator (struct fields)
pub fn emit_tip_event(
    env: &Env,
    event_id: u64,
    sender: &Address,
    creator: &Address,
    amount: i128,
    token: &Address,
) {
    let event = TipEvent {
        version: EVENT_VERSION,
        event_id,
        timestamp: env.ledger().timestamp(),
        sender: sender.clone(),
        creator: creator.clone(),
        amount,
        token: token.clone(),
    };

    env.events().publish(
        (symbol_short!("tip"), creator.clone()),
        event,
    );
}

/// Emit a structured withdraw event
/// 
/// # Event Schema
/// - **Topic**: `withdraw` (symbol)
/// - **Data**: WithdrawEvent struct with named fields (creator, amount, token, timestamp)
/// - **Filterable By**: creator (topics)
pub fn emit_withdraw_event(
    env: &Env,
    creator: &Address,
    amount: i128,
    token: &Address,
) {
    let event = WithdrawEvent {
        version: EVENT_VERSION,
        creator: creator.clone(),
        amount,
        token: token.clone(),
        timestamp: env.ledger().timestamp(),
    };

    env.events().publish(
        (symbol_short!("withdraw"), creator.clone()),
        event,
    );
}

