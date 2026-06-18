//! Tip Reputation System
//!
//! On-chain reputation scores derived from tipping history and behaviour.
//!
//! # Score model
//! Each account has a `ReputationScore` that is:
//! - **Increased** when the account tips (tipper) or receives tips (creator).
//! - **Decayed** exponentially over time: score halves every `HALF_LIFE_SECS`.
//! - **Rewarded** with bonus points for consistent tipping streaks.
//!
//! Score is stored as a fixed-point integer (PRECISION = 1_000_000 = 1.0).
//! History is stored as a bounded ring-buffer of `REP_HISTORY_SIZE` entries.

use soroban_sdk::{contracttype, Address, Env, Vec};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Fixed-point precision (1_000_000 = 1.0 reputation point).
pub const PRECISION: i128 = 1_000_000;

/// Score awarded to a tipper per unit of token tipped (scaled by PRECISION).
/// 1 reputation point per 1_000 token base units.
pub const TIPPER_SCORE_PER_UNIT: i128 = 1_000; // 1 point per 1_000 units

/// Score awarded to a creator per unit of token received.
pub const CREATOR_SCORE_PER_UNIT: i128 = 500; // 0.5 points per 1_000 units

/// Half-life for score decay: score halves every 30 days.
pub const HALF_LIFE_SECS: u64 = 30 * 24 * 3_600; // 30 days

/// Streak bonus: extra score multiplier (BPS) per consecutive-day tip.
/// 100 bps = 1% bonus per streak day, capped at `MAX_STREAK_DAYS`.
pub const STREAK_BONUS_BPS_PER_DAY: u32 = 100;

/// Maximum streak days counted for bonus.
pub const MAX_STREAK_DAYS: u32 = 30;

/// Window for streak continuity: tips within 25–49 hours count as next-day.
pub const STREAK_WINDOW_SECS: u64 = 49 * 3_600;

/// Number of history entries stored per account (ring-buffer size).
pub const REP_HISTORY_SIZE: u32 = 20;

/// Minimum score (floor — never decays below this).
pub const MIN_SCORE: i128 = 0;

// ── Types ────────────────────────────────────────────────────────────────────

/// Reputation score for an account.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReputationScore {
    /// Account this score belongs to.
    pub account: Address,
    /// Current score (PRECISION scale).
    pub score: i128,
    /// Timestamp of the last score update (for decay calculation).
    pub last_updated: u64,
    /// Current tipping streak in days.
    pub streak_days: u32,
    /// Timestamp of the last tip sent (for streak tracking).
    pub last_tip_at: u64,
    /// Total tips sent (count).
    pub tips_sent: u64,
    /// Total tips received (count).
    pub tips_received: u64,
    /// Cumulative amount tipped (token base units).
    pub total_tipped: i128,
    /// Cumulative amount received (token base units).
    pub total_received: i128,
}

/// A single reputation history entry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepHistoryEntry {
    /// Score delta (positive = gain, negative = decay applied).
    pub delta: i128,
    /// Score after this event.
    pub score_after: i128,
    /// Reason for the change.
    pub reason: RepReason,
    /// Timestamp.
    pub timestamp: u64,
}

/// Reason for a reputation change.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum RepReason {
    /// Account sent a tip.
    TipSent,
    /// Account received a tip.
    TipReceived,
    /// Streak bonus applied.
    StreakBonus,
    /// Periodic decay applied.
    Decay,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

/// Get reputation score for an account (defaults to zero).
pub fn get_score(env: &Env, account: &Address) -> ReputationScore {
    env.storage()
        .persistent()
        .get(&DataKey::ReputationScore(account.clone()))
        .unwrap_or(ReputationScore {
            account: account.clone(),
            score: 0,
            last_updated: env.ledger().timestamp(),
            streak_days: 0,
            last_tip_at: 0,
            tips_sent: 0,
            tips_received: 0,
            total_tipped: 0,
            total_received: 0,
        })
}

fn save_score(env: &Env, rep: &ReputationScore) {
    env.storage()
        .persistent()
        .set(&DataKey::ReputationScore(rep.account.clone()), rep);
}

fn get_history(env: &Env, account: &Address) -> Vec<RepHistoryEntry> {
    env.storage()
        .persistent()
        .get(&DataKey::ReputationHistory(account.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

fn push_history(env: &Env, account: &Address, entry: RepHistoryEntry) {
    let mut hist = get_history(env, account);
    if hist.len() >= REP_HISTORY_SIZE {
        // Drop oldest entry (index 0)
        let mut trimmed: Vec<RepHistoryEntry> = Vec::new(env);
        for i in 1..hist.len() {
            trimmed.push_back(hist.get(i).unwrap());
        }
        hist = trimmed;
    }
    hist.push_back(entry);
    env.storage()
        .persistent()
        .set(&DataKey::ReputationHistory(account.clone()), &hist);
}

// ── Core logic ───────────────────────────────────────────────────────────────

/// Apply time-based decay to a score and return the decayed value.
///
/// Uses the formula: `score * 2^(-elapsed / HALF_LIFE)` approximated as
/// repeated halving for whole half-life periods plus a linear interpolation
/// for the remainder — all in integer arithmetic.
pub fn apply_decay(score: i128, elapsed_secs: u64) -> i128 {
    if score <= 0 || elapsed_secs == 0 {
        return score.max(MIN_SCORE);
    }

    let mut s = score;
    let mut remaining = elapsed_secs;

    // Apply whole half-life periods
    while remaining >= HALF_LIFE_SECS && s > 0 {
        s /= 2;
        remaining -= HALF_LIFE_SECS;
    }

    // Linear interpolation for the fractional period:
    // s * (1 - remaining/HALF_LIFE * 0.5) = s - s * remaining / (2 * HALF_LIFE)
    if remaining > 0 && s > 0 {
        let decay_frac = s * (remaining as i128) / (2 * HALF_LIFE_SECS as i128);
        s = (s - decay_frac).max(0);
    }

    s.max(MIN_SCORE)
}

/// Compute the streak bonus multiplier in BPS (10_000 = 1.0x, 10_100 = 1.01x).
pub fn streak_bonus_bps(streak_days: u32) -> u32 {
    let days = streak_days.min(MAX_STREAK_DAYS);
    10_000 + days * STREAK_BONUS_BPS_PER_DAY
}

/// Record that `account` sent a tip of `amount` token units.
/// Updates score, streak, and history.
pub fn record_tip_sent(env: &Env, account: &Address, amount: i128) {
    let now = env.ledger().timestamp();
    let mut rep = get_score(env, account);

    // Decay existing score
    let elapsed = now.saturating_sub(rep.last_updated);
    rep.score = apply_decay(rep.score, elapsed);

    // Base score gain
    let base_gain = amount / TIPPER_SCORE_PER_UNIT;

    // Streak update
    let streak_updated = update_streak(&mut rep, now);

    // Apply streak bonus to gain
    let bonus_bps = streak_bonus_bps(rep.streak_days) as i128;
    let gain = base_gain * bonus_bps / 10_000;

    rep.score += gain;
    rep.tips_sent += 1;
    rep.total_tipped += amount;
    rep.last_updated = now;

    push_history(
        env,
        account,
        RepHistoryEntry {
            delta: gain,
            score_after: rep.score,
            reason: RepReason::TipSent,
            timestamp: now,
        },
    );

    if streak_updated {
        push_history(
            env,
            account,
            RepHistoryEntry {
                delta: 0,
                score_after: rep.score,
                reason: RepReason::StreakBonus,
                timestamp: now,
            },
        );
    }

    save_score(env, &rep);
}

/// Record that `account` received a tip of `amount` token units.
pub fn record_tip_received(env: &Env, account: &Address, amount: i128) {
    let now = env.ledger().timestamp();
    let mut rep = get_score(env, account);

    let elapsed = now.saturating_sub(rep.last_updated);
    rep.score = apply_decay(rep.score, elapsed);

    let gain = amount / CREATOR_SCORE_PER_UNIT;
    rep.score += gain;
    rep.tips_received += 1;
    rep.total_received += amount;
    rep.last_updated = now;

    push_history(
        env,
        account,
        RepHistoryEntry {
            delta: gain,
            score_after: rep.score,
            reason: RepReason::TipReceived,
            timestamp: now,
        },
    );

    save_score(env, &rep);
}

/// Explicitly trigger decay for an account (e.g. called by a keeper).
/// Records a history entry for the decay event.
pub fn trigger_decay(env: &Env, account: &Address) {
    let now = env.ledger().timestamp();
    let mut rep = get_score(env, account);

    let elapsed = now.saturating_sub(rep.last_updated);
    if elapsed == 0 {
        return;
    }

    let old_score = rep.score;
    rep.score = apply_decay(rep.score, elapsed);
    rep.last_updated = now;

    let delta = rep.score - old_score; // negative
    push_history(
        env,
        account,
        RepHistoryEntry {
            delta,
            score_after: rep.score,
            reason: RepReason::Decay,
            timestamp: now,
        },
    );

    save_score(env, &rep);
}

/// Get the reputation history for an account.
pub fn get_reputation_history(env: &Env, account: &Address) -> Vec<RepHistoryEntry> {
    get_history(env, account)
}

// ── Internal ─────────────────────────────────────────────────────────────────

/// Update streak counter. Returns true if streak was extended.
fn update_streak(rep: &mut ReputationScore, now: u64) -> bool {
    if rep.last_tip_at == 0 {
        rep.streak_days = 1;
        rep.last_tip_at = now;
        return true;
    }

    let elapsed = now.saturating_sub(rep.last_tip_at);

    if elapsed <= STREAK_WINDOW_SECS {
        // Same streak window — extend streak
        rep.streak_days = (rep.streak_days + 1).min(MAX_STREAK_DAYS);
        rep.last_tip_at = now;
        true
    } else {
        // Streak broken — reset
        rep.streak_days = 1;
        rep.last_tip_at = now;
        false
    }
}
