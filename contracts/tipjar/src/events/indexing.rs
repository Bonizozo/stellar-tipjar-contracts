//! Event indexing for efficient querying

use super::{DataKey, EventFilter, TipEvent};
use soroban_sdk::{Address, Env, String, Vec};

/// Update event indexes for efficient querying
pub fn update_event_index(env: &Env, event: &TipEvent) {
    // Update creator index
    let creator_key = DataKey::CreatorEvents(event.creator.clone());
    let mut creator_events: Vec<u64> = env
        .storage()
        .persistent()
        .get(&creator_key)
        .unwrap_or_else(|| Vec::new(env));
    creator_events.push_back(event.event_id);
    env.storage().persistent().set(&creator_key, &creator_events);

    // Update sender index
    let sender_key = DataKey::SenderEvents(event.sender.clone());
    let mut sender_events: Vec<u64> = env
        .storage()
        .persistent()
        .get(&sender_key)
        .unwrap_or_else(|| Vec::new(env));
    sender_events.push_back(event.event_id);
    env.storage().persistent().set(&sender_key, &sender_events);

    // Update token index
    let token_key = DataKey::TokenEvents(event.token.clone());
    let mut token_events: Vec<u64> = env
        .storage()
        .persistent()
        .get(&token_key)
        .unwrap_or_else(|| Vec::new(env));
    token_events.push_back(event.event_id);
    env.storage().persistent().set(&token_key, &token_events);

    // Update time index (by day)
    let day_timestamp = event.timestamp / 86400 * 86400; // Round down to day
    let time_key = DataKey::TimeEvents(day_timestamp);
    let mut time_events: Vec<u64> = env
        .storage()
        .persistent()
        .get(&time_key)
        .unwrap_or_else(|| Vec::new(env));
    time_events.push_back(event.event_id);
    env.storage().persistent().set(&time_key, &time_events);

    // Update tag indexes
    for tag in event.tags.iter() {
        let tag_key = DataKey::TagEvents(tag.clone());
        let mut tag_events: Vec<u64> = env
            .storage()
            .persistent()
            .get(&tag_key)
            .unwrap_or_else(|| Vec::new(env));
        tag_events.push_back(event.event_id);
        env.storage().persistent().set(&tag_key, &tag_events);
    }

    // Update event counter
    let counter_key = DataKey::EventCounter;
    let current_counter: u64 = env.storage().persistent().get(&counter_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&counter_key, &(current_counter + 1));
}

/// Get event IDs matching filter criteria
pub fn get_event_ids(env: &Env, filter: &EventFilter) -> Vec<u64> {
    let mut result: Vec<u64> = Vec::new(env);

    // Start with the most specific index
    if let Some(ref creator) = filter.creator {
        let creator_key = DataKey::CreatorEvents(creator.clone());
        let creator_events: Vec<u64> = env
            .storage()
            .persistent()
            .get(&creator_key)
            .unwrap_or_else(|| Vec::new(env));
        for event_id in creator_events.iter() {
            result.push_back(event_id);
        }
    } else if let Some(ref sender) = filter.sender {
        let sender_key = DataKey::SenderEvents(sender.clone());
        let sender_events: Vec<u64> = env
            .storage()
            .persistent()
            .get(&sender_key)
            .unwrap_or_else(|| Vec::new(env));
        for event_id in sender_events.iter() {
            result.push_back(event_id);
        }
    } else if let Some(ref token) = filter.token {
        let token_key = DataKey::TokenEvents(token.clone());
        let token_events: Vec<u64> = env
            .storage()
            .persistent()
            .get(&token_key)
            .unwrap_or_else(|| Vec::new(env));
        for event_id in token_events.iter() {
            result.push_back(event_id);
        }
    } else if let Some(start) = filter.start_time {
        // Get events from time range
        let start_day = start / 86400 * 86400;
        let end_day = if let Some(end) = filter.end_time {
            end / 86400 * 86400
        } else {
            env.ledger().timestamp() / 86400 * 86400
        };

        let mut day = start_day;
        while day <= end_day {
            let time_key = DataKey::TimeEvents(day);
            let day_events: Vec<u64> = env
                .storage()
                .persistent()
                .get(&time_key)
                .unwrap_or_else(|| Vec::new(env));
            for event_id in day_events.iter() {
                result.push_back(event_id);
            }
            day += 86400;
        }
    } else if let Some(ref tags) = filter.tags {
        if tags.len() > 0 {
            // Use first tag for initial filtering
            let first_tag = tags.get(0).unwrap();
            let tag_key = DataKey::TagEvents(first_tag);
            let tag_events: Vec<u64> = env
                .storage()
                .persistent()
                .get(&tag_key)
                .unwrap_or_else(|| Vec::new(env));
            for event_id in tag_events.iter() {
                result.push_back(event_id);
            }
        }
    } else {
        // No specific filter, get all events
        let counter_key = DataKey::EventCounter;
        let total_events: u64 = env.storage().persistent().get(&counter_key).unwrap_or(0);
        for i in 0..total_events {
            result.push_back(i);
        }
    }

    result
}

/// Get next event ID
pub fn get_next_event_id(env: &Env) -> u64 {
    let counter_key = DataKey::EventCounter;
    env.storage().persistent().get(&counter_key).unwrap_or(0)
}
