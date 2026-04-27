//! Lottery entry logic.
//!
//! Tippers are automatically entered into the active lottery round when they
//! send a qualifying tip. Each qualifying tip grants one entry; larger tips
//! grant additional entries proportional to the tip amount.

use soroban_sdk::{Address, Env, Vec};

use crate::DataKey;

use super::{LotteryEntry, LotteryStatus, MAX_ENTRIES_PER_TIPPER, MIN_TIP_FOR_ENTRY};

// ── Public API ───────────────────────────────────────────────────────────────

/// Records lottery entries for a tipper based on their tip amount.
///
/// Called automatically when a tip is sent. Entries are only recorded when:
/// - There is an active (Open) lottery round.
/// - The tip amount meets the minimum threshold.
///
/// Entry count = `tip_amount / MIN_TIP_FOR_ENTRY`, capped at
/// `MAX_ENTRIES_PER_TIPPER` per round.
///
/// Returns the number of entries granted (0 if no active round or tip too small).
pub fn record_tip_entry(env: &Env, tipper: &Address, tip_amount: i128) -> u32 {
    if tip_amount < MIN_TIP_FOR_ENTRY {
        return 0;
    }

    let round_id = match get_active_round_id(env) {
        Some(id) => id,
        None => return 0,
    };

    let round: super::LotteryRound = match env
        .storage()
        .persistent()
        .get(&DataKey::LotteryRound(round_id))
    {
        Some(r) => r,
        None => return 0,
    };

    if round.status != LotteryStatus::Open {
        return 0;
    }

    // Check round is within time window
    let now = env.ledger().timestamp();
    if now < round.start_time || now > round.end_time {
        return 0;
    }

    // Tip must meet this round's minimum
    if tip_amount < round.min_tip_amount {
        return 0;
    }

    // Calculate entries: 1 per MIN_TIP_FOR_ENTRY, capped at MAX_ENTRIES_PER_TIPPER
    let raw_entries = (tip_amount / MIN_TIP_FOR_ENTRY) as u32;

    let mut entry = get_entry(env, round_id, tipper).unwrap_or(LotteryEntry {
        round_id,
        tipper: tipper.clone(),
        entry_count: 0,
        total_tipped: 0,
        first_entry_at: now,
        last_entry_at: now,
    });

    let remaining_capacity = MAX_ENTRIES_PER_TIPPER.saturating_sub(entry.entry_count);
    let entries_to_add = raw_entries.min(remaining_capacity);

    if entries_to_add == 0 {
        return 0;
    }

    entry.entry_count += entries_to_add;
    entry.total_tipped += tip_amount;
    entry.last_entry_at = now;

    save_entry(env, round_id, &entry);
    track_round_tipper(env, round_id, tipper);

    // Update round total entries
    let mut updated_round = round;
    updated_round.total_entries += entries_to_add as u64;
    env.storage()
        .persistent()
        .set(&DataKey::LotteryRound(round_id), &updated_round);

    env.events().publish(
        (soroban_sdk::symbol_short!("lot_entr"),),
        (tipper.clone(), round_id, entries_to_add, entry.entry_count),
    );

    entries_to_add
}

/// Returns the lottery entry for `(round_id, tipper)`, or `None`.
pub fn get_entry(env: &Env, round_id: u64, tipper: &Address) -> Option<LotteryEntry> {
    env.storage()
        .persistent()
        .get(&DataKey::LotteryEntry(round_id, tipper.clone()))
}

/// Returns all tipper addresses that have entries in `round_id`.
pub fn get_round_tippers(env: &Env, round_id: u64) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::LotteryRoundTippers(round_id))
        .unwrap_or_else(|| Vec::new(env))
}

/// Returns the currently active round ID, or `None`.
pub fn get_active_round_id(env: &Env) -> Option<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::LotteryActiveRound)
}

/// Sets the active round ID.
pub(super) fn set_active_round_id(env: &Env, round_id: Option<u64>) {
    match round_id {
        Some(id) => env
            .storage()
            .persistent()
            .set(&DataKey::LotteryActiveRound, &id),
        None => env
            .storage()
            .persistent()
            .remove(&DataKey::LotteryActiveRound),
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────────

pub(super) fn save_entry(env: &Env, round_id: u64, entry: &LotteryEntry) {
    env.storage().persistent().set(
        &DataKey::LotteryEntry(round_id, entry.tipper.clone()),
        entry,
    );
}

fn track_round_tipper(env: &Env, round_id: u64, tipper: &Address) {
    let mut tippers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::LotteryRoundTippers(round_id))
        .unwrap_or_else(|| Vec::new(env));
    if !tippers.contains(tipper) {
        tippers.push_back(tipper.clone());
        env.storage()
            .persistent()
            .set(&DataKey::LotteryRoundTippers(round_id), &tippers);
    }
}


