//! Conviction Voting System
//!
//! Implements conviction voting where voting power accumulates over time.
//! Voters who lock tokens for longer periods gain more voting power on proposals.

use soroban_sdk::{contracttype, Address, Env};

/// Conviction vote record with time-based accumulation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConvictionVote {
    pub voter: Address,
    pub proposal_id: u64,
    pub base_voting_power: i128, // Initial voting power (token amount)
    pub conviction_start: u64,   // When conviction started accumulating
    pub last_updated: u64,       // Last time conviction was recalculated
    pub accumulated_conviction: i128, // Total conviction accumulated
}

/// Conviction voting configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConvictionConfig {
    pub conviction_period: u64, // Period to reach max conviction (seconds)
    pub max_conviction_multiplier: i128, // Max voting power multiplier (e.g., 3_000_000 for 3x)
    pub conviction_decay_rate_bps: u32, // Decay rate in basis points per second when vote changes
    pub min_conviction_threshold: i128, // Minimum conviction to vote
}

/// Storage keys for conviction voting
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConvictionDataKey {
    ConvictionVote(u64, Address), // proposal_id, voter
    ConvictionConfig,
    ConvictionHistory(u64, Address), // proposal_id, voter - historical records
    VoterConvictionTotal(Address),   // Total conviction across all votes
}

/// Default conviction voting constants
pub const DEFAULT_CONVICTION_PERIOD: u64 = 2592000; // 30 days
pub const DEFAULT_MAX_CONVICTION_MULTIPLIER: i128 = 3_000_000; // 3x
pub const DEFAULT_CONVICTION_DECAY_RATE_BPS: u32 = 100; // 0.01% per second
pub const DEFAULT_MIN_CONVICTION_THRESHOLD: i128 = 100_000; // Minimum 0.1 tokens

/// Initialize conviction voting configuration
pub fn init_conviction_voting(env: &Env) {
    let config = ConvictionConfig {
        conviction_period: DEFAULT_CONVICTION_PERIOD,
        max_conviction_multiplier: DEFAULT_MAX_CONVICTION_MULTIPLIER,
        conviction_decay_rate_bps: DEFAULT_CONVICTION_DECAY_RATE_BPS,
        min_conviction_threshold: DEFAULT_MIN_CONVICTION_THRESHOLD,
    };

    env.storage()
        .persistent()
        .set(&ConvictionDataKey::ConvictionConfig, &config);
}

/// Get conviction voting configuration
pub fn get_conviction_config(env: &Env) -> ConvictionConfig {
    env.storage()
        .persistent()
        .get(&ConvictionDataKey::ConvictionConfig)
        .expect("Conviction voting not initialized")
}

/// Update conviction voting configuration
pub fn update_conviction_config(env: &Env, config: &ConvictionConfig) {
    env.storage()
        .persistent()
        .set(&ConvictionDataKey::ConvictionConfig, config);
}

/// Calculate conviction multiplier based on time locked
/// Returns multiplier as fixed-point number (1_000_000 = 1x)
pub fn calculate_conviction_multiplier(env: &Env, conviction_start: u64) -> i128 {
    let now = env.ledger().timestamp();
    let time_locked = now.saturating_sub(conviction_start);

    let config = get_conviction_config(env);

    if time_locked >= config.conviction_period {
        // Maximum conviction reached
        config.max_conviction_multiplier
    } else if time_locked == 0 {
        // No time locked yet
        1_000_000 // 1x multiplier
    } else {
        // Linear interpolation: multiplier = 1 + (time_locked / conviction_period) * (max - 1)
        let progress = (time_locked as i128) * 1_000_000 / config.conviction_period as i128;
        let max_gain = config.max_conviction_multiplier - 1_000_000;
        1_000_000 + (progress * max_gain / 1_000_000)
    }
}

/// Calculate effective voting power with conviction multiplier
pub fn calculate_effective_voting_power(env: &Env, conviction_vote: &ConvictionVote) -> i128 {
    let multiplier = calculate_conviction_multiplier(env, conviction_vote.conviction_start);
    conviction_vote.base_voting_power * multiplier / 1_000_000
}

/// Calculate conviction accumulated over time
pub fn calculate_accumulated_conviction(env: &Env, conviction_vote: &ConvictionVote) -> i128 {
    let now = env.ledger().timestamp();
    let time_since_last_update = now.saturating_sub(conviction_vote.last_updated);

    if time_since_last_update == 0 {
        return conviction_vote.accumulated_conviction;
    }

    // Conviction accumulates as: base_power * time_locked / conviction_period
    let config = get_conviction_config(env);
    let _time_locked = now.saturating_sub(conviction_vote.conviction_start);

    // Calculate conviction rate per second
    let conviction_rate =
        conviction_vote.base_voting_power * 1_000_000 / config.conviction_period as i128;
    let new_conviction = conviction_rate * time_since_last_update as i128 / 1_000_000;

    conviction_vote.accumulated_conviction + new_conviction
}

/// Record a conviction vote on a proposal
pub fn record_conviction_vote(
    env: &Env,
    voter: &Address,
    proposal_id: u64,
    base_voting_power: i128,
) {
    voter.require_auth();

    let config = get_conviction_config(env);

    if base_voting_power < config.min_conviction_threshold {
        panic!("Voting power below minimum conviction threshold");
    }

    let now = env.ledger().timestamp();
    let conviction_vote = ConvictionVote {
        voter: voter.clone(),
        proposal_id,
        base_voting_power,
        conviction_start: now,
        last_updated: now,
        accumulated_conviction: 0,
    };

    let vote_key = ConvictionDataKey::ConvictionVote(proposal_id, voter.clone());
    env.storage().persistent().set(&vote_key, &conviction_vote);

    // Update voter's total conviction
    let total_key = ConvictionDataKey::VoterConvictionTotal(voter.clone());
    let current_total: i128 = env.storage().persistent().get(&total_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&total_key, &(current_total + base_voting_power));
}

/// Get conviction vote for a voter on a proposal
pub fn get_conviction_vote(env: &Env, proposal_id: u64, voter: &Address) -> Option<ConvictionVote> {
    let vote_key = ConvictionDataKey::ConvictionVote(proposal_id, voter.clone());
    env.storage().persistent().get(&vote_key)
}

/// Update conviction vote (e.g., when voter changes their vote)
pub fn update_conviction_vote(
    env: &Env,
    voter: &Address,
    proposal_id: u64,
    new_base_voting_power: i128,
) {
    voter.require_auth();

    let config = get_conviction_config(env);

    if new_base_voting_power < config.min_conviction_threshold {
        panic!("New voting power below minimum conviction threshold");
    }

    let vote_key = ConvictionDataKey::ConvictionVote(proposal_id, voter.clone());
    let mut conviction_vote = env
        .storage()
        .persistent()
        .get::<_, ConvictionVote>(&vote_key)
        .expect("No conviction vote found");

    // Apply decay to accumulated conviction when vote changes
    let decay_rate = config.conviction_decay_rate_bps as i128;
    let time_since_vote = env
        .ledger()
        .timestamp()
        .saturating_sub(conviction_vote.last_updated);
    let decay_amount =
        conviction_vote.accumulated_conviction * decay_rate * time_since_vote as i128
            / 10_000
            / 1_000_000;
    conviction_vote.accumulated_conviction = conviction_vote
        .accumulated_conviction
        .saturating_sub(decay_amount);

    // Update vote
    conviction_vote.base_voting_power = new_base_voting_power;
    conviction_vote.last_updated = env.ledger().timestamp();

    env.storage().persistent().set(&vote_key, &conviction_vote);

    // Update voter's total conviction
    let total_key = ConvictionDataKey::VoterConvictionTotal(voter.clone());
    let current_total: i128 = env.storage().persistent().get(&total_key).unwrap_or(0);
    let old_power = env
        .storage()
        .persistent()
        .get(&vote_key)
        .map(|v: ConvictionVote| v.base_voting_power)
        .unwrap_or(0);
    let new_total = current_total - old_power + new_base_voting_power;
    env.storage().persistent().set(&total_key, &new_total);
}

/// Get total conviction for a voter across all proposals
pub fn get_voter_total_conviction(env: &Env, voter: &Address) -> i128 {
    let total_key = ConvictionDataKey::VoterConvictionTotal(voter.clone());
    env.storage().persistent().get(&total_key).unwrap_or(0)
}

/// Check if voter meets minimum conviction threshold for a proposal
pub fn meets_conviction_threshold(env: &Env, proposal_id: u64, voter: &Address) -> bool {
    if let Some(conviction_vote) = get_conviction_vote(env, proposal_id, voter) {
        let config = get_conviction_config(env);
        conviction_vote.base_voting_power >= config.min_conviction_threshold
    } else {
        false
    }
}

/// Get proposal threshold based on conviction voting
pub fn get_proposal_threshold_with_conviction(env: &Env, voter: &Address) -> i128 {
    let total_conviction = get_voter_total_conviction(env, voter);
    let config = get_conviction_config(env);

    // Threshold is reduced based on conviction: threshold = base_threshold * (1 - conviction_bonus)
    // conviction_bonus = min(total_conviction / (base_threshold * 10), 0.5)
    let conviction_bonus =
        (total_conviction * 1_000_000 / (config.min_conviction_threshold * 10)).min(500_000); // Max 50% reduction

    config.min_conviction_threshold * (1_000_000 - conviction_bonus) / 1_000_000
}

/// Track conviction vote history for auditing
pub fn record_conviction_history(
    env: &Env,
    proposal_id: u64,
    voter: &Address,
    conviction_vote: &ConvictionVote,
) {
    let history_key = ConvictionDataKey::ConvictionHistory(proposal_id, voter.clone());
    env.storage()
        .persistent()
        .set(&history_key, conviction_vote);
}

/// Get conviction vote history
pub fn get_conviction_history(
    env: &Env,
    proposal_id: u64,
    voter: &Address,
) -> Option<ConvictionVote> {
    let history_key = ConvictionDataKey::ConvictionHistory(proposal_id, voter.clone());
    env.storage().persistent().get(&history_key)
}
