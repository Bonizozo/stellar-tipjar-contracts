//! Reputation Tokens module (#310).
//!
//! Non-transferable reputation tokens tracking creator and supporter behaviour.
//! Supports minting, decay, score calculation, and history tracking.

use soroban_sdk::{contracttype, symbol_short, Address, Env, Vec};

use crate::DataKey;

/// Fixed-point precision (1_000_000 = 1.0).
pub const PRECISION: i128 = 1_000_000;

/// Score awarded per unit tipped.
pub const SCORE_PER_UNIT: i128 = 1_000;

/// Half-life for decay: score halves every 30 days.
pub const HALF_LIFE_SECS: u64 = 30 * 24 * 3_600;

/// Max history entries per account.
pub const HISTORY_SIZE: u32 = 20;

/// A reputation token record (non-transferable).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReputationToken {
    pub owner: Address,
    /// Current score (PRECISION scale).
    pub score: i128,
    pub last_updated: u64,
    pub total_minted: i128,
    pub total_decayed: i128,
}

/// A history entry for reputation changes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepTokenHistory {
    pub delta: i128,
    pub score_after: i128,
    pub timestamp: u64,
    pub is_decay: bool,
}

/// Reputation token storage sub-keys.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepTokenKey {
    Token(Address),
    History(Address),
}

/// Apply time-based decay to a score.
pub fn apply_decay(score: i128, elapsed_secs: u64) -> i128 {
    if score <= 0 || elapsed_secs == 0 {
        return score.max(0);
    }
    let mut s = score;
    let mut remaining = elapsed_secs;
    while remaining >= HALF_LIFE_SECS && s > 0 {
        s /= 2;
        remaining -= HALF_LIFE_SECS;
    }
    if remaining > 0 && s > 0 {
        let decay_frac = s * (remaining as i128) / (2 * HALF_LIFE_SECS as i128);
        s = (s - decay_frac).max(0);
    }
    s.max(0)
}

/// Mint reputation tokens for an account based on tip amount.
pub fn mint(env: &Env, account: &Address, amount: i128) {
    let now = env.ledger().timestamp();
    let mut token = load_token(env, account, now);

    let elapsed = now.saturating_sub(token.last_updated);
    let old_score = token.score;
    token.score = apply_decay(token.score, elapsed);

    let gain = amount / SCORE_PER_UNIT;
    token.score += gain;
    token.total_minted += gain;
    token.last_updated = now;

    push_history(
        env,
        account,
        RepTokenHistory {
            delta: gain,
            score_after: token.score,
            timestamp: now,
            is_decay: false,
        },
    );

    if old_score != token.score - gain {
        let decay_delta = (token.score - gain) - old_score;
        if decay_delta != 0 {
            push_history(
                env,
                account,
                RepTokenHistory {
                    delta: decay_delta,
                    score_after: token.score - gain,
                    timestamp: now,
                    is_decay: true,
                },
            );
            token.total_decayed += decay_delta.abs();
        }
    }

    save_token(env, &token);

    env.events().publish(
        (symbol_short!("rep_mint"),),
        (account.clone(), gain, token.score),
    );
}

/// Trigger explicit decay for an account.
pub fn decay(env: &Env, account: &Address) {
    let now = env.ledger().timestamp();
    let mut token = load_token(env, account, now);

    let elapsed = now.saturating_sub(token.last_updated);
    if elapsed == 0 {
        return;
    }

    let old_score = token.score;
    token.score = apply_decay(token.score, elapsed);
    let delta = token.score - old_score;
    token.total_decayed += delta.abs();
    token.last_updated = now;

    push_history(
        env,
        account,
        RepTokenHistory {
            delta,
            score_after: token.score,
            timestamp: now,
            is_decay: true,
        },
    );

    save_token(env, &token);

    env.events().publish(
        (symbol_short!("rep_dcy"),),
        (account.clone(), delta, token.score),
    );
}

/// Get the current reputation score for an account.
pub fn get_score(env: &Env, account: &Address) -> i128 {
    let now = env.ledger().timestamp();
    let token = load_token(env, account, now);
    let elapsed = now.saturating_sub(token.last_updated);
    apply_decay(token.score, elapsed)
}

/// Get the full reputation token record.
pub fn get_token(env: &Env, account: &Address) -> ReputationToken {
    let now = env.ledger().timestamp();
    load_token(env, account, now)
}

/// Get reputation history for an account.
pub fn get_history(env: &Env, account: &Address) -> Vec<RepTokenHistory> {
    env.storage()
        .persistent()
        .get(&DataKey::RepToken(RepTokenKey::History(account.clone())))
        .unwrap_or_else(|| Vec::new(env))
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn load_token(env: &Env, account: &Address, now: u64) -> ReputationToken {
    env.storage()
        .persistent()
        .get(&DataKey::RepToken(RepTokenKey::Token(account.clone())))
        .unwrap_or(ReputationToken {
            owner: account.clone(),
            score: 0,
            last_updated: now,
            total_minted: 0,
            total_decayed: 0,
        })
}

fn save_token(env: &Env, token: &ReputationToken) {
    env.storage().persistent().set(
        &DataKey::RepToken(RepTokenKey::Token(token.owner.clone())),
        token,
    );
}

fn push_history(env: &Env, account: &Address, entry: RepTokenHistory) {
    let mut hist: Vec<RepTokenHistory> = env
        .storage()
        .persistent()
        .get(&DataKey::RepToken(RepTokenKey::History(account.clone())))
        .unwrap_or_else(|| Vec::new(env));
    if hist.len() >= HISTORY_SIZE {
        let mut trimmed: Vec<RepTokenHistory> = Vec::new(env);
        for i in 1..hist.len() {
            trimmed.push_back(hist.get(i).unwrap());
        }
        hist = trimmed;
    }
    hist.push_back(entry);
    env.storage().persistent().set(
        &DataKey::RepToken(RepTokenKey::History(account.clone())),
        &hist,
    );
}
