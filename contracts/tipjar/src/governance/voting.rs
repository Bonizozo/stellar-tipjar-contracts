//! Voting mechanism for governance

use super::{Vote, VoteChoice, DataKey};
use soroban_sdk::{Address, Env};

/// Cast a vote on a proposal
pub fn cast_vote(
    env: &Env,
    voter: &Address,
    proposal_id: u64,
    choice: VoteChoice,
    voting_power: i128,
) {
    voter.require_auth();

    if voting_power <= 0 {
        panic!("Voting power must be positive");
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

    // Record vote
    let vote = Vote {
        voter: voter.clone(),
        proposal_id,
        choice: choice.clone(),
        voting_power,
        timestamp: env.ledger().timestamp(),
    };

    env.storage().persistent().set(&vote_key, &vote);

    // Update voter's total votes
    let voter_votes_key = DataKey::VoterVotes(voter.clone());
    let current_votes: i128 = env.storage().persistent().get(&voter_votes_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&voter_votes_key, &(current_votes + voting_power));

    // Update proposal vote totals
    let mut votes_for = proposal.votes_for;
    let mut votes_against = proposal.votes_against;

    match choice {
        VoteChoice::For => votes_for += voting_power,
        VoteChoice::Against => votes_against += voting_power,
        VoteChoice::Abstain => {
            // Abstain votes don't count towards for/against
        }
    }

    super::proposals::update_proposal_votes(env, proposal_id, votes_for, votes_against);
}

/// Get vote for a specific voter on a proposal
pub fn get_vote(env: &Env, proposal_id: u64, voter: &Address) -> Option<Vote> {
    let vote_key = DataKey::Vote(proposal_id, voter.clone());
    env.storage().persistent().get(&vote_key)
}

/// Get total votes cast by a voter
pub fn get_voter_total_votes(env: &Env, voter: &Address) -> i128 {
    let voter_votes_key = DataKey::VoterVotes(voter.clone());
    env.storage().persistent().get(&voter_votes_key).unwrap_or(0)
}

/// Get all votes for a proposal
pub fn get_proposal_votes(env: &Env, proposal_id: u64) -> Vec<Vote> {
    let mut votes: Vec<Vote> = Vec::new(env);
    let proposal = super::proposals::get_proposal_or_panic(env, proposal_id);

    // This is a simplified implementation
    // In production, you'd want to iterate through all voters
    // For now, we'll return an empty vector as iteration is complex without knowing all voters

    votes
}

/// Check if a voter has voted on a proposal
pub fn has_voted(env: &Env, proposal_id: u64, voter: &Address) -> bool {
    let vote_key = DataKey::Vote(proposal_id, voter.clone());
    env.storage().persistent().has(&vote_key)
}

/// Get voting power for a voter (simplified - returns stored value)
pub fn get_voting_power(env: &Env, voter: &Address) -> i128 {
    // In a real implementation, this would check token balance
    // For now, return the stored voting power
    get_voter_total_votes(env, voter)
}

/// Get proposal vote breakdown
pub fn get_vote_breakdown(env: &Env, proposal_id: u64) -> (i128, i128, i128) {
    let proposal = super::proposals::get_proposal_or_panic(env, proposal_id);
    let total_votes = proposal.votes_for + proposal.votes_against;
    (proposal.votes_for, proposal.votes_against, total_votes)
}
