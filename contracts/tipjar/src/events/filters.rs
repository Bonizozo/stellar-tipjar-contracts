//! Event filtering utilities

use super::{EventFilter, TipEvent};
use soroban_sdk::Vec;

/// Check if an event matches the given filter
pub fn matches_filter(event: &TipEvent, filter: &EventFilter) -> bool {
    // Check sender filter
    if let Some(ref sender) = filter.sender {
        if event.sender != *sender {
            return false;
        }
    }

    // Check creator filter
    if let Some(ref creator) = filter.creator {
        if event.creator != *creator {
            return false;
        }
    }

    // Check token filter
    if let Some(ref token) = filter.token {
        if event.token != *token {
            return false;
        }
    }

    // Check amount filters
    if let Some(min) = filter.min_amount {
        if event.amount < min {
            return false;
        }
    }

    if let Some(max) = filter.max_amount {
        if event.amount > max {
            return false;
        }
    }

    // Check time filters
    if let Some(start) = filter.start_time {
        if event.timestamp < start {
            return false;
        }
    }

    if let Some(end) = filter.end_time {
        if event.timestamp > end {
            return false;
        }
    }

    // Check tags filter
    if let Some(ref filter_tags) = filter.tags {
        if filter_tags.len() > 0 {
            let mut has_tag = false;
            for filter_tag in filter_tags.iter() {
                for event_tag in event.tags.iter() {
                    if filter_tag == event_tag {
                        has_tag = true;
                        break;
                    }
                }
                if has_tag {
                    break;
                }
            }
            if !has_tag {
                return false;
            }
        }
    }

    true
}

/// Create a filter for events by creator
pub fn creator_filter(creator: &soroban_sdk::Address) -> EventFilter {
    EventFilter {
        event_type: None,
        sender: None,
        creator: Some(creator.clone()),
        token: None,
        min_amount: None,
        max_amount: None,
        start_time: None,
        end_time: None,
        tags: None,
    }
}

/// Create a filter for events by sender
pub fn sender_filter(sender: &soroban_sdk::Address) -> EventFilter {
    EventFilter {
        event_type: None,
        sender: Some(sender.clone()),
        creator: None,
        token: None,
        min_amount: None,
        max_amount: None,
        start_time: None,
        end_time: None,
        tags: None,
    }
}

/// Create a filter for events by token
pub fn token_filter(token: &soroban_sdk::Address) -> EventFilter {
    EventFilter {
        event_type: None,
        sender: None,
        creator: None,
        token: Some(token.clone()),
        min_amount: None,
        max_amount: None,
        start_time: None,
        end_time: None,
        tags: None,
    }
}

/// Create a filter for events by time range
pub fn time_range_filter(start: u64, end: u64) -> EventFilter {
    EventFilter {
        event_type: None,
        sender: None,
        creator: None,
        token: None,
        min_amount: None,
        max_amount: None,
        start_time: Some(start),
        end_time: Some(end),
        tags: None,
    }
}

/// Create a filter for events by amount range
pub fn amount_range_filter(min: i128, max: i128) -> EventFilter {
    EventFilter {
        event_type: None,
        sender: None,
        creator: None,
        token: None,
        min_amount: Some(min),
        max_amount: Some(max),
        start_time: None,
        end_time: None,
        tags: None,
    }
}

/// Create a filter for events by tags
pub fn tags_filter(tags: &Vec<soroban_sdk::String>) -> EventFilter {
    EventFilter {
        event_type: None,
        sender: None,
        creator: None,
        token: None,
        min_amount: None,
        max_amount: None,
        start_time: None,
        end_time: None,
        tags: Some(tags.clone()),
    }
}
