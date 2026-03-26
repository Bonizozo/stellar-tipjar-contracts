//! Advanced Event System with Filtering and Indexing
//!
//! This module provides enhanced event tracking, filtering, and querying capabilities
//! for the TipJar contract.

pub mod filters;
pub mod indexing;

use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Vec};

/// Enhanced event structure with versioning and metadata
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipEvent {
    pub version: u32,
    pub event_id: u64,
    pub timestamp: u64,
    pub sender: Address,
    pub creator: Address,
    pub amount: i128,
    pub token: Address,
    pub message: Option<String>,
    pub tags: Vec<String>,
}

/// Event types for categorization
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventType {
    Tip,
    TipWithMessage,
    TipLocked,
    Withdraw,
    WithdrawLocked,
    MatchCreated,
    MatchApplied,
    MatchCancelled,
}

/// Event filter for querying
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventFilter {
    pub event_type: Option<EventType>,
    pub sender: Option<Address>,
    pub creator: Option<Address>,
    pub token: Option<Address>,
    pub min_amount: Option<i128>,
    pub max_amount: Option<i128>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub tags: Option<Vec<String>>,
}

/// Event query parameters
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventQuery {
    pub filter: EventFilter,
    pub limit: u32,
    pub offset: u32,
}

/// Current event version
pub const EVENT_VERSION: u32 = 1;

/// Emit a tip event with enhanced metadata
pub fn emit_tip_event(
    env: &Env,
    event_id: u64,
    sender: &Address,
    creator: &Address,
    amount: i128,
    token: &Address,
    message: Option<&String>,
    tags: Option<&Vec<String>>,
) {
    let event = TipEvent {
        version: EVENT_VERSION,
        event_id,
        timestamp: env.ledger().timestamp(),
        sender: sender.clone(),
        creator: creator.clone(),
        amount,
        token: token.clone(),
        message: message.cloned(),
        tags: tags.cloned().unwrap_or_else(|| Vec::new(env)),
    };

    // Store event in persistent storage for querying
    let event_key = DataKey::Event(event_id);
    env.storage().persistent().set(&event_key, &event);

    // Update event index
    indexing::update_event_index(env, &event);

    // Emit event for external listeners
    env.events().publish(
        (symbol_short!("tip"), event.version, event.event_id),
        (
            event.sender.clone(),
            event.creator.clone(),
            event.amount,
            event.token.clone(),
            event.timestamp,
        ),
    );
}

/// Get event by ID
pub fn get_event(env: &Env, event_id: u64) -> Option<TipEvent> {
    let event_key = DataKey::Event(event_id);
    env.storage().persistent().get(&event_key)
}

/// Query events with filters
pub fn query_events(env: &Env, query: &EventQuery) -> Vec<TipEvent> {
    let limit = if query.limit == 0 || query.limit > 100 {
        100
    } else {
        query.limit
    };

    // Get all event IDs from index
    let event_ids = indexing::get_event_ids(env, &query.filter);

    let mut results: Vec<TipEvent> = Vec::new(env);
    let mut count = 0u32;
    let mut skipped = 0u32;

    for event_id in event_ids.iter() {
        if skipped < query.offset {
            skipped += 1;
            continue;
        }

        if count >= limit {
            break;
        }

        if let Some(event) = get_event(env, event_id) {
            if filters::matches_filter(&event, &query.filter) {
                results.push_back(event);
                count += 1;
            }
        }
    }

    results
}

/// Get events by creator
pub fn get_events_by_creator(
    env: &Env,
    creator: &Address,
    from_id: u64,
    limit: u32,
) -> Vec<TipEvent> {
    let filter = EventFilter {
        event_type: None,
        sender: None,
        creator: Some(creator.clone()),
        token: None,
        min_amount: None,
        max_amount: None,
        start_time: None,
        end_time: None,
        tags: None,
    };

    let query = EventQuery {
        filter,
        limit,
        offset: 0,
    };

    query_events(env, &query)
}

/// Get events by time range
pub fn get_events_by_timerange(
    env: &Env,
    start: u64,
    end: u64,
    limit: u32,
) -> Vec<TipEvent> {
    let filter = EventFilter {
        event_type: None,
        sender: None,
        creator: None,
        token: None,
        min_amount: None,
        max_amount: None,
        start_time: Some(start),
        end_time: Some(end),
        tags: None,
    };

    let query = EventQuery {
        filter,
        limit,
        offset: 0,
    };

    query_events(env, &query)
}

/// Get events by sender
pub fn get_events_by_sender(
    env: &Env,
    sender: &Address,
    limit: u32,
) -> Vec<TipEvent> {
    let filter = EventFilter {
        event_type: None,
        sender: Some(sender.clone()),
        creator: None,
        token: None,
        min_amount: None,
        max_amount: None,
        start_time: None,
        end_time: None,
        tags: None,
    };

    let query = EventQuery {
        filter,
        limit,
        offset: 0,
    };

    query_events(env, &query)
}

/// Get events by token
pub fn get_events_by_token(
    env: &Env,
    token: &Address,
    limit: u32,
) -> Vec<TipEvent> {
    let filter = EventFilter {
        event_type: None,
        sender: None,
        creator: None,
        token: Some(token.clone()),
        min_amount: None,
        max_amount: None,
        start_time: None,
        end_time: None,
        tags: None,
    };

    let query = EventQuery {
        filter,
        limit,
        offset: 0,
    };

    query_events(env, &query)
}

/// Storage keys for event system
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Event(u64),
    EventCounter,
    CreatorEvents(Address),
    SenderEvents(Address),
    TokenEvents(Address),
    TimeEvents(u64),
    TagEvents(String),
}
