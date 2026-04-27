//! Random winner selection for lottery rounds.
//!
//! Uses the Soroban ledger's `sequence_number` combined with the round ID and
//! a nonce to derive pseudo-random winner indices. This is deterministic given
//! the same ledger state, which is acceptable for a tip-bonus lottery where
//! the stakes are low and the randomness source is transparent.
//!
//! Selection algorithm:
//! 1. Build a weighted ticket list: each tipper appears once per entry.
//! 2. Derive a seed from `ledger_sequence XOR round_id XOR nonce`.
//! 3. Use a linear-congruential generator to pick indices without replacement.

use soroban_sdk::{panic_with_error, Address, Env, Vec};

use crate::{DataKey, LotteryError, TipJarError};

use super::{
    entries::{get_entry, get_round_tippers, set_active_round_id},
    LotteryStatus, LotteryWinner,
};

// ── Public API ───────────────────────────────────────────────────────────────

/// Closes the round for new entries and selects winners.
///
/// Can only be called by admin after `round.end_time` has passed.
/// Transitions the round from `Open` → `Drawing` → `Completed`.
///
/// Returns the list of winners (addresses and prize amounts).
///
/// Panics if:
/// - Round not found.
/// - Round is not `Open`.
/// - End time has not passed.
/// - No entries in the round.
pub fn draw_winners(env: &Env, admin: &Address, round_id: u64) -> Vec<LotteryWinner> {
    admin.require_auth();
    require_admin(env, admin);

    let mut round: super::LotteryRound = env
        .storage()
        .persistent()
        .get(&DataKey::LotteryRound(round_id))
        .unwrap_or_else(|| panic_with_error!(env, LotteryError::LotteryRoundNotFound));

    if round.status != LotteryStatus::Open {
        panic_with_error!(env, LotteryError::LotteryRoundNotOpen);
    }

    let now = env.ledger().timestamp();
    if now <= round.end_time {
        panic_with_error!(env, LotteryError::LotteryRoundNotEnded);
    }

    if round.total_entries == 0 {
        // No entries — cancel the round
        round.status = LotteryStatus::Cancelled;
        env.storage()
            .persistent()
            .set(&DataKey::LotteryRound(round_id), &round);
        set_active_round_id(env, None);
        env.events().publish(
            (soroban_sdk::symbol_short!("lot_cncl"),),
            (round_id, soroban_sdk::symbol_short!("no_entr")),
        );
        return Vec::new(env);
    }

    // Mark as Drawing
    round.status = LotteryStatus::Drawing;
    env.storage()
        .persistent()
        .set(&DataKey::LotteryRound(round_id), &round);

    // Build weighted ticket list
    let tippers = get_round_tippers(env, round_id);
    let mut tickets: Vec<Address> = Vec::new(env);
    for tipper in tippers.iter() {
        if let Some(entry) = get_entry(env, round_id, &tipper) {
            for _ in 0..entry.entry_count {
                tickets.push_back(tipper.clone());
            }
        }
    }

    let total_tickets = tickets.len() as u64;
    if total_tickets == 0 {
        round.status = LotteryStatus::Cancelled;
        env.storage()
            .persistent()
            .set(&DataKey::LotteryRound(round_id), &round);
        set_active_round_id(env, None);
        return Vec::new(env);
    }

    // Select winners using pseudo-random LCG seeded from ledger sequence
    let winner_count = round.winner_count.min(
        // Can't have more winners than unique tippers
        get_round_tippers(env, round_id).len() as u32,
    );

    let prize_amounts_arr =
        super::prizes::calculate_prize_amounts(round.prize_pool, winner_count);

    let seed = derive_seed(env, round_id);
    let selected = select_without_replacement(env, &tickets, winner_count, seed);

    // Build winner records and persist them
    let mut winners: Vec<LotteryWinner> = Vec::new(env);
    let mut winner_list: Vec<Address> = Vec::new(env);

    for (position, winner_addr) in selected.iter().enumerate() {
        let prize = prize_amounts_arr.get(position).copied().unwrap_or(0);
        let winner = LotteryWinner {
            round_id,
            winner: winner_addr.clone(),
            prize_amount: prize,
            position: position as u32 + 1,
            claimed_at: None,
        };
        env.storage().persistent().set(
            &DataKey::LotteryWinner(round_id, position as u32 + 1),
            &winner,
        );
        winner_list.push_back(winner_addr.clone());
        winners.push_back(winner);
    }

    // Persist winner list for the round
    env.storage()
        .persistent()
        .set(&DataKey::LotteryRoundWinners(round_id), &winner_list);

    // Mark round as Completed
    round.status = LotteryStatus::Completed;
    round.drawn_at = Some(now);
    env.storage()
        .persistent()
        .set(&DataKey::LotteryRound(round_id), &round);

    // Clear active round
    set_active_round_id(env, None);

    env.events().publish(
        (soroban_sdk::symbol_short!("lot_draw"),),
        (round_id, winner_count, round.prize_pool),
    );

    winners
}

/// Returns the list of winner addresses for a completed round.
pub fn get_round_winners(env: &Env, round_id: u64) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::LotteryRoundWinners(round_id))
        .unwrap_or_else(|| Vec::new(env))
}

/// Returns a specific winner record by round and position (1-indexed).
pub fn get_winner(env: &Env, round_id: u64, position: u32) -> Option<LotteryWinner> {
    env.storage()
        .persistent()
        .get(&DataKey::LotteryWinner(round_id, position))
}

// ── Pseudo-random helpers ────────────────────────────────────────────────────

/// Derives a u64 seed from the current ledger sequence and round ID.
fn derive_seed(env: &Env, round_id: u64) -> u64 {
    let seq = env.ledger().sequence() as u64;
    let ts = env.ledger().timestamp();
    seq.wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
        ^ ts.wrapping_mul(2862933555777941757)
        ^ round_id.wrapping_mul(3935559000370003845)
}

/// Selects `count` unique addresses from `tickets` (weighted by repetition)
/// without replacement using a linear-congruential generator.
fn select_without_replacement(
    env: &Env,
    tickets: &Vec<Address>,
    count: u32,
    seed: u64,
) -> Vec<Address> {
    let mut selected: Vec<Address> = Vec::new(env);
    let mut selected_indices: Vec<u64> = Vec::new(env);
    let total = tickets.len() as u64;

    let mut rng = seed;
    let mut attempts = 0u32;
    let max_attempts = count.saturating_mul(20).max(200);

    while (selected.len() as u32) < count && attempts < max_attempts {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);

        let idx = rng % total;

        if selected_indices.contains(&idx) {
            attempts += 1;
            continue;
        }

        let winner_addr = tickets.get(idx as u32).unwrap();

        if selected.contains(&winner_addr) {
            selected_indices.push_back(idx);
            attempts += 1;
            continue;
        }

        selected_indices.push_back(idx);
        selected.push_back(winner_addr);
        attempts = 0;
    }

    selected
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn require_admin(env: &Env, admin: &Address) {
    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::Unauthorized));
    if *admin != stored_admin {
        panic_with_error!(env, TipJarError::Unauthorized);
    }
}
