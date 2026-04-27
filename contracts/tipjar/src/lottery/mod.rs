//! Lottery system where tippers are entered into drawings for bonus rewards.
//!
//! This module implements a lottery mechanism where:
//! - Each tip above a minimum threshold automatically enters the tipper into a drawing
//! - Admins can create lottery rounds with prize pools
//! - Random winner selection using on-chain randomness
//! - Prize distribution to winners
//! - Multiple winners per round (configurable)

pub mod drawing;
pub mod entries;
pub mod prizes;

use soroban_sdk::{contracttype, Address};

// ── Constants ────────────────────────────────────────────────────────────────

/// Minimum tip amount to qualify for lottery entry (1 token with 7 decimals).
pub const MIN_TIP_FOR_ENTRY: i128 = 10_000_000;

/// Maximum number of winners per lottery round.
pub const MAX_WINNERS_PER_ROUND: u32 = 10;

/// Maximum number of entries per tipper per round (prevents spam).
pub const MAX_ENTRIES_PER_TIPPER: u32 = 100;

// ── Data types ───────────────────────────────────────────────────────────────

/// Status of a lottery round.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LotteryStatus {
    /// Round is open for entries.
    Open,
    /// Round is closed, drawing in progress.
    Drawing,
    /// Round is completed, prizes distributed.
    Completed,
    /// Round was cancelled by admin.
    Cancelled,
}

/// A lottery round configuration and state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LotteryRound {
    /// Unique round ID.
    pub id: u64,
    /// Token used for prize pool.
    pub prize_token: Address,
    /// Total prize pool amount.
    pub prize_pool: i128,
    /// Number of winners to select.
    pub winner_count: u32,
    /// Minimum tip amount to qualify for entry.
    pub min_tip_amount: i128,
    /// Timestamp when the round opens.
    pub start_time: u64,
    /// Timestamp when the round closes.
    pub end_time: u64,
    /// Current status.
    pub status: LotteryStatus,
    /// Total number of entries in this round.
    pub total_entries: u64,
    /// Timestamp when the round was created.
    pub created_at: u64,
    /// Timestamp when winners were drawn (if completed).
    pub drawn_at: Option<u64>,
}

/// A lottery entry for a specific tipper in a round.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LotteryEntry {
    /// Round ID.
    pub round_id: u64,
    /// Tipper address.
    pub tipper: Address,
    /// Number of entries this tipper has in this round.
    pub entry_count: u32,
    /// Total amount tipped by this tipper in this round.
    pub total_tipped: i128,
    /// Timestamp of first entry.
    pub first_entry_at: u64,
    /// Timestamp of last entry.
    pub last_entry_at: u64,
}

/// A lottery winner record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LotteryWinner {
    /// Round ID.
    pub round_id: u64,
    /// Winner address.
    pub winner: Address,
    /// Prize amount awarded.
    pub prize_amount: i128,
    /// Winner's position (1st, 2nd, 3rd, etc.).
    pub position: u32,
    /// Timestamp when prize was claimed.
    pub claimed_at: Option<u64>,
}
