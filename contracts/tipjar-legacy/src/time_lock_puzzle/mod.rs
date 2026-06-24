//! Tip Time-Lock Puzzles
//!
//! Implements cryptographic time-lock puzzles for delayed tip reveals and
//! scheduled releases. A puzzle wraps a tip amount behind a computational
//! challenge that becomes solvable only after a target unlock time.

pub mod puzzle;
pub mod solver;

use soroban_sdk::{contracttype, Address, BytesN, Env};

// ── Types ────────────────────────────────────────────────────────────────────

/// Status of a time-lock puzzle.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PuzzleStatus {
    /// Puzzle is active and not yet solved.
    Active,
    /// Puzzle has been solved and tip released.
    Solved,
    /// Puzzle was cancelled by the creator before solving.
    Cancelled,
}

/// Difficulty tier controlling the number of required hash iterations.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PuzzleDifficulty {
    /// ~1 000 iterations — suitable for short delays (minutes).
    Easy,
    /// ~10 000 iterations — suitable for medium delays (hours).
    Medium,
    /// ~100 000 iterations — suitable for long delays (days).
    Hard,
}

/// A time-lock puzzle wrapping a tip.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeLockPuzzle {
    /// Unique puzzle identifier.
    pub id: u64,
    /// Address that created (and funded) the puzzle.
    pub creator: Address,
    /// Intended tip recipient.
    pub recipient: Address,
    /// Token used for the tip.
    pub token: Address,
    /// Tip amount locked inside the puzzle.
    pub amount: i128,
    /// Puzzle commitment: SHA-256(secret || nonce).
    pub commitment: BytesN<32>,
    /// Number of sequential hash iterations required to solve.
    pub iterations: u64,
    /// Ledger timestamp before which the puzzle cannot be solved.
    pub unlock_time: u64,
    /// Ledger timestamp at which the puzzle was created.
    pub created_at: u64,
    /// Difficulty tier used when generating this puzzle.
    pub difficulty: PuzzleDifficulty,
    /// Current lifecycle status.
    pub status: PuzzleStatus,
}

/// Storage keys scoped to the time-lock puzzle module.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Puzzle record by ID.
    Puzzle(u64),
    /// Global puzzle counter.
    PuzzleCounter,
    /// List of puzzle IDs created by an address.
    CreatorPuzzles(Address),
    /// List of puzzle IDs targeting a recipient.
    RecipientPuzzles(Address),
}

// ── Constants ────────────────────────────────────────────────────────────────

/// Iteration count for Easy difficulty.
pub const EASY_ITERATIONS: u64 = 1_000;
/// Iteration count for Medium difficulty.
pub const MEDIUM_ITERATIONS: u64 = 10_000;
/// Iteration count for Hard difficulty.
pub const HARD_ITERATIONS: u64 = 100_000;

// ── Storage helpers ──────────────────────────────────────────────────────────

/// Fetch a puzzle by ID, returning `None` if not found.
pub fn get_puzzle(env: &Env, id: u64) -> Option<TimeLockPuzzle> {
    env.storage().persistent().get(&DataKey::Puzzle(id))
}

/// Persist a puzzle record.
pub fn save_puzzle(env: &Env, puzzle: &TimeLockPuzzle) {
    env.storage()
        .persistent()
        .set(&DataKey::Puzzle(puzzle.id), puzzle);
}

/// Allocate the next puzzle ID and increment the counter.
pub fn next_puzzle_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::PuzzleCounter)
        .unwrap_or(0)
        + 1;
    env.storage().persistent().set(&DataKey::PuzzleCounter, &id);
    id
}

/// Append a puzzle ID to the creator's list.
pub fn add_creator_puzzle(env: &Env, creator: &Address, puzzle_id: u64) {
    let mut list: soroban_sdk::Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::CreatorPuzzles(creator.clone()))
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));
    list.push_back(puzzle_id);
    env.storage()
        .persistent()
        .set(&DataKey::CreatorPuzzles(creator.clone()), &list);
}

/// Append a puzzle ID to the recipient's list.
pub fn add_recipient_puzzle(env: &Env, recipient: &Address, puzzle_id: u64) {
    let mut list: soroban_sdk::Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::RecipientPuzzles(recipient.clone()))
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));
    list.push_back(puzzle_id);
    env.storage()
        .persistent()
        .set(&DataKey::RecipientPuzzles(recipient.clone()), &list);
}

/// Return all puzzle IDs created by `creator`.
pub fn get_creator_puzzles(env: &Env, creator: &Address) -> soroban_sdk::Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::CreatorPuzzles(creator.clone()))
        .unwrap_or_else(|| soroban_sdk::Vec::new(env))
}

/// Return all puzzle IDs targeting `recipient`.
pub fn get_recipient_puzzles(env: &Env, recipient: &Address) -> soroban_sdk::Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::RecipientPuzzles(recipient.clone()))
        .unwrap_or_else(|| soroban_sdk::Vec::new(env))
}

/// Map a `PuzzleDifficulty` to its iteration count.
pub fn difficulty_iterations(difficulty: PuzzleDifficulty) -> u64 {
    match difficulty {
        PuzzleDifficulty::Easy => EASY_ITERATIONS,
        PuzzleDifficulty::Medium => MEDIUM_ITERATIONS,
        PuzzleDifficulty::Hard => HARD_ITERATIONS,
    }
}
