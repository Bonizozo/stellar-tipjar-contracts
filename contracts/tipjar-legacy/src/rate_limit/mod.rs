//! Rate limiting for the tipping system.
//!
//! Prevents spam and abuse by tracking tip counts and amounts per time window.
//! Supports configurable limits with different tiers (Default, Verified, Premium).

use soroban_sdk::{contracttype, symbol_short, Address, Env};

use crate::DataKey;

/// Rate limit tier — higher tiers get more generous limits.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RateLimitTier {
    /// Standard limits for all users.
    Default,
    /// Relaxed limits for verified users.
    Verified,
    /// Highest limits for premium users.
    Premium,
}

/// Rate limit configuration for a tier.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateLimitConfig {
    /// Maximum number of tips per window.
    pub max_tips_per_window: u32,
    /// Maximum total amount tipped per window.
    pub max_amount_per_window: i128,
    /// Window duration in seconds (e.g. 3600 = 1 hour).
    pub window_seconds: u64,
    /// Minimum seconds between consecutive tips from the same sender.
    pub min_tip_interval: u64,
}

/// Per-sender rate limit state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateLimitState {
    /// Number of tips sent in the current window.
    pub tips_in_window: u32,
    /// Total amount tipped in the current window.
    pub amount_in_window: i128,
    /// Timestamp when the current window started.
    pub window_start: u64,
    /// Timestamp of the last tip.
    pub last_tip_time: u64,
}

/// Violation kind returned by `check_limits`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Violation {
    Interval,
    TipCount,
    Amount,
}

/// Default rate limit config (applied when no tier-specific config exists).
pub fn default_config() -> RateLimitConfig {
    RateLimitConfig {
        max_tips_per_window: 20,
        max_amount_per_window: 1_000_000_000, // 1 billion base units
        window_seconds: 3_600,                // 1 hour
        min_tip_interval: 0,                  // no per-tip cooldown by default
    }
}

/// Get the rate limit config for a tier.
pub fn get_config(env: &Env, tier: &RateLimitTier) -> RateLimitConfig {
    env.storage()
        .persistent()
        .get(&DataKey::RateLimitConfig(tier.clone()))
        .unwrap_or_else(default_config)
}

/// Get the current rate limit state for a sender.
pub fn get_state(env: &Env, sender: &Address) -> RateLimitState {
    env.storage()
        .persistent()
        .get(&DataKey::RateLimitState(sender.clone()))
        .unwrap_or(RateLimitState {
            tips_in_window: 0,
            amount_in_window: 0,
            window_start: 0,
            last_tip_time: 0,
        })
}

/// Get the rate limit tier for a sender.
pub fn get_sender_tier(env: &Env, sender: &Address) -> RateLimitTier {
    env.storage()
        .persistent()
        .get(&DataKey::RateLimitTier(sender.clone()))
        .unwrap_or(RateLimitTier::Default)
}

/// Check rate limits for a sender tipping `amount`.
///
/// Returns `Ok(())` if within limits, or `Err(Violation)` if a limit is exceeded.
/// Does NOT update state — call `record_tip` after a successful check.
pub fn check_limits(env: &Env, sender: &Address, amount: i128) -> Result<(), Violation> {
    let tier = get_sender_tier(env, sender);
    let config = get_config(env, &tier);
    let now = env.ledger().timestamp();
    let mut state = get_state(env, sender);

    // Reset window if expired.
    if now >= state.window_start + config.window_seconds {
        state.tips_in_window = 0;
        state.amount_in_window = 0;
    }

    // Check per-tip interval.
    if config.min_tip_interval > 0 && state.last_tip_time > 0 {
        if now < state.last_tip_time + config.min_tip_interval {
            return Err(Violation::Interval);
        }
    }

    // Check tip count limit.
    if state.tips_in_window >= config.max_tips_per_window {
        return Err(Violation::TipCount);
    }

    // Check amount limit.
    if state.amount_in_window + amount > config.max_amount_per_window {
        return Err(Violation::Amount);
    }

    Ok(())
}

/// Record a tip after a successful limit check, updating the sender's state.
pub fn record_tip(env: &Env, sender: &Address, amount: i128) {
    let tier = get_sender_tier(env, sender);
    let config = get_config(env, &tier);
    let now = env.ledger().timestamp();
    let mut state = get_state(env, sender);

    // Reset window if expired.
    if now >= state.window_start + config.window_seconds {
        state.tips_in_window = 0;
        state.amount_in_window = 0;
        state.window_start = now;
    }

    state.tips_in_window += 1;
    state.amount_in_window += amount;
    state.last_tip_time = now;

    env.storage()
        .persistent()
        .set(&DataKey::RateLimitState(sender.clone()), &state);

    env.events().publish(
        (symbol_short!("rl_rec"),),
        (sender.clone(), state.tips_in_window, state.amount_in_window),
    );
}
