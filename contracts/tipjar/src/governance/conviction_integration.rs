//! Integration layer between conviction voting and standard voting
//!
//! This module provides functions to cast votes using conviction voting power
//! and integrates with the existing proposal and voting systems.

use super::conviction::{
    self, calculate_effective_voting_power, get_conviction_vote, record_conviction_vote,
    update_conviction_vote, ConvictionConfig, ConvictionVote,
};
use super::voting;
use super::{DataKey, Vote, VoteChoice};
use soroban_sdk::{Address, Env};

/// Cast a conviction vote on a proposal
/// The voter's voting power accumulates over time based on how long they've locked tokens
pub fn cast_conviction_vote(
    env: &Env,
    voter: &Address,
    proposal_id: u64,
    choice: VoteChoice,
    base_voting_power: i128,
) {
    voter.require_auth();

    // Validate base voting power
    let config = conviction::get_conviction_config(env);
    if base_voting_power < config.min_conviction_threshold {
        panic!("Voting power below minimum conviction threshold");
    }

    // Check if proposal exists and is active
    let proposal = super::proposals::get_proposal_or_panic(env, proposal_id);
    if !super::proposals::is_proposal_active(env, proposal_id) {
        panic!("Proposal is not active");
    }

    // Check if voter already voted
    let vote_key = DataKey::Vote(proposal_id, voter.clone());
    if env.storage().persistent().has(&vote_key) {
        panic!("Already voted on this proposal");
    }

    // Record conviction vote
    record_conviction_vote(env, voter, proposal_id, base_voting_power);

    // Calculate effective voting power with conviction multiplier
    let conviction_vote =
        get_conviction_vote(env, proposal_id, voter).expect("Conviction vote should exist");
    let effective_voting_power = calculate_effective_voting_power(env, &conviction_vote);

    // Record standard vote with effective voting power
    let vote = Vote {
        voter: voter.clone(),
        proposal_id,
        choice: choice.clone(),
        voting_power: effective_voting_power,
        timestamp: env.ledger().timestamp(),
    };

    env.storage().persistent().set(&vote_key, &vote);

    // Update voter's total votes
    let voter_votes_key = DataKey::VoterVotes(voter.clone());
    let current_votes: i128 = env
        .storage()
        .persistent()
        .get(&voter_votes_key)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&voter_votes_key, &(current_votes + effective_voting_power));

    // Update proposal vote totals
    let mut votes_for = proposal.votes_for;
    let mut votes_against = proposal.votes_against;

    match choice {
        VoteChoice::For => votes_for += effective_voting_power,
        VoteChoice::Against => votes_against += effective_voting_power,
        VoteChoice::Abstain => {
            // Abstain votes don't count towards for/against
        }
    }

    super::proposals::update_proposal_votes(env, proposal_id, votes_for, votes_against);

    // Emit conviction vote event
    env.events().publish(
        (soroban_sdk::symbol_short!("conv_vote"),),
        (voter.clone(), proposal_id, effective_voting_power),
    );
}

/// Change a conviction vote (e.g., voter wants to increase their voting power)
/// This applies decay to accumulated conviction as a penalty
pub fn change_conviction_vote(
    env: &Env,
    voter: &Address,
    proposal_id: u64,
    new_choice: VoteChoice,
    new_base_voting_power: i128,
) {
    voter.require_auth();

    let config = conviction::get_conviction_config(env);
    if new_base_voting_power < config.min_conviction_threshold {
        panic!("New voting power below minimum conviction threshold");
    }

    // Get existing vote
    let vote_key = DataKey::Vote(proposal_id, voter.clone());
    let old_vote = env
        .storage()
        .persistent()
        .get::<_, Vote>(&vote_key)
        .expect("No vote found to change");

    // Get existing conviction vote
    let old_conviction =
        get_conviction_vote(env, proposal_id, voter).expect("No conviction vote found");

    // Update conviction vote (applies decay)
    update_conviction_vote(env, voter, proposal_id, new_base_voting_power);

    // Get updated conviction vote
    let new_conviction =
        get_conviction_vote(env, proposal_id, voter).expect("Conviction vote should exist");
    let new_effective_voting_power = calculate_effective_voting_power(env, &new_conviction);

    // Update standard vote
    let mut new_vote = old_vote.clone();
    new_vote.choice = new_choice.clone();
    new_vote.voting_power = new_effective_voting_power;
    new_vote.timestamp = env.ledger().timestamp();

    env.storage().persistent().set(&vote_key, &new_vote);

    // Update voter's total votes
    let voter_votes_key = DataKey::VoterVotes(voter.clone());
    let current_votes: i128 = env
        .storage()
        .persistent()
        .get(&voter_votes_key)
        .unwrap_or(0);
    let vote_diff = new_effective_voting_power - old_vote.voting_power;
    env.storage()
        .persistent()
        .set(&voter_votes_key, &(current_votes + vote_diff));

    // Update proposal vote totals
    let proposal = super::proposals::get_proposal_or_panic(env, proposal_id);
    let mut votes_for = proposal.votes_for;
    let mut votes_against = proposal.votes_against;

    // Remove old vote contribution
    match old_vote.choice {
        VoteChoice::For => votes_for -= old_vote.voting_power,
        VoteChoice::Against => votes_against -= old_vote.voting_power,
        VoteChoice::Abstain => {}
    }

    // Add new vote contribution
    match new_choice {
        VoteChoice::For => votes_for += new_effective_voting_power,
        VoteChoice::Against => votes_against += new_effective_voting_power,
        VoteChoice::Abstain => {}
    }

    super::proposals::update_proposal_votes(env, proposal_id, votes_for, votes_against);

    // Emit conviction vote change event
    env.events().publish(
        (soroban_sdk::symbol_short!("conv_chg"),),
        (voter.clone(), proposal_id, new_effective_voting_power),
    );
}

/// Get effective voting power for a voter on a proposal (with conviction multiplier)
pub fn get_effective_voting_power(env: &Env, proposal_id: u64, voter: &Address) -> i128 {
    if let Some(conviction_vote) = get_conviction_vote(env, proposal_id, voter) {
        calculate_effective_voting_power(env, &conviction_vote)
    } else {
        0
    }
}

/// Get conviction voting details for a voter on a proposal
pub fn get_conviction_voting_details(
    env: &Env,
    proposal_id: u64,
    voter: &Address,
) -> Option<ConvictionVotingDetails> {
    if let Some(conviction_vote) = get_conviction_vote(env, proposal_id, voter) {
        let multiplier =
            conviction::calculate_conviction_multiplier(env, conviction_vote.conviction_start);
        let accumulated = conviction::calculate_accumulated_conviction(env, &conviction_vote);
        let effective_power = calculate_effective_voting_power(env, &conviction_vote);

        Some(ConvictionVotingDetails {
            base_voting_power: conviction_vote.base_voting_power,
            conviction_start: conviction_vote.conviction_start,
            conviction_multiplier: multiplier,
            accumulated_conviction: accumulated,
            effective_voting_power: effective_power,
            time_locked: env
                .ledger()
                .timestamp()
                .saturating_sub(conviction_vote.conviction_start),
        })
    } else {
        None
    }
}

/// Details about a voter's conviction voting status
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConvictionVotingDetails {
    pub base_voting_power: i128,
    pub conviction_start: u64,
    pub conviction_multiplier: i128,
    pub accumulated_conviction: i128,
    pub effective_voting_power: i128,
    pub time_locked: u64,
}

/// Get proposal threshold adjusted by voter's conviction
pub fn get_adjusted_proposal_threshold(env: &Env, voter: &Address) -> i128 {
    conviction::get_proposal_threshold_with_conviction(env, voter)
}

/// Check if voter can create a proposal based on conviction voting
pub fn can_create_proposal_with_conviction(env: &Env, voter: &Address) -> bool {
    let threshold = get_adjusted_proposal_threshold(env, voter);
    let total_conviction = conviction::get_voter_total_conviction(env, voter);
    total_conviction >= threshold
}
