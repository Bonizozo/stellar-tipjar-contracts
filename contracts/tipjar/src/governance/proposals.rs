//! Proposal management for governance

use super::{Proposal, ProposalAction, DataKey};
use soroban_sdk::{Address, Env, String, Vec};

/// Create a new proposal
pub fn create_proposal(
    env: &Env,
    proposer: &Address,
    description: &String,
    actions: &Vec<ProposalAction>,
) -> u64 {
    proposer.require_auth();

    if actions.is_empty() {
        panic!("Proposal must have at least one action");
    }

    let config = super::get_governance_config(env);

    // Check proposer has minimum tokens (simplified - in production, check token balance)
    // For now, we'll skip the balance check as it requires token contract interaction

    let proposal_id = super::get_next_proposal_id(env);
    let now = env.ledger().timestamp();

    let proposal = Proposal {
        id: proposal_id,
        proposer: proposer.clone(),
        description: description.clone(),
        actions: actions.clone(),
        start_time: now + config.voting_delay,
        end_time: now + config.voting_delay + config.voting_period,
        votes_for: 0,
        votes_against: 0,
        executed: false,
        canceled: false,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Proposal(proposal_id), &proposal);

    super::increment_proposal_count(env);

    proposal_id
}

/// Get proposal by ID
pub fn get_proposal(env: &Env, proposal_id: u64) -> Option<Proposal> {
    env.storage()
        .persistent()
        .get(&DataKey::Proposal(proposal_id))
}

/// Get proposal (panics if not found)
pub fn get_proposal_or_panic(env: &Env, proposal_id: u64) -> Proposal {
    get_proposal(env, proposal_id).expect("Proposal not found")
}

/// Update proposal votes
pub fn update_proposal_votes(
    env: &Env,
    proposal_id: u64,
    votes_for: i128,
    votes_against: i128,
) {
    let mut proposal = get_proposal_or_panic(env, proposal_id);
    proposal.votes_for = votes_for;
    proposal.votes_against = votes_against;
    env.storage()
        .persistent()
        .set(&DataKey::Proposal(proposal_id), &proposal);
}

/// Mark proposal as executed
pub fn mark_proposal_executed(env: &Env, proposal_id: u64) {
    let mut proposal = get_proposal_or_panic(env, proposal_id);
    proposal.executed = true;
    env.storage()
        .persistent()
        .set(&DataKey::Proposal(proposal_id), &proposal);
}

/// Mark proposal as canceled
pub fn mark_proposal_canceled(env: &Env, proposal_id: u64) {
    let mut proposal = get_proposal_or_panic(env, proposal_id);
    proposal.canceled = true;
    env.storage()
        .persistent()
        .set(&DataKey::Proposal(proposal_id), &proposal);
}

/// Check if proposal is active (voting period)
pub fn is_proposal_active(env: &Env, proposal_id: u64) -> bool {
    let proposal = get_proposal_or_panic(env, proposal_id);
    let now = env.ledger().timestamp();

    !proposal.canceled && !proposal.executed && now >= proposal.start_time && now <= proposal.end_time
}

/// Check if proposal has passed
pub fn has_proposal_passed(env: &Env, proposal_id: u64) -> bool {
    let proposal = get_proposal_or_panic(env, proposal_id);
    let config = super::get_governance_config(env);

    if proposal.canceled || proposal.executed {
        return false;
    }

    let now = env.ledger().timestamp();
    if now <= proposal.end_time {
        return false; // Voting still ongoing
    }

    // Check quorum
    let total_votes = proposal.votes_for + proposal.votes_against;
    if total_votes < config.quorum {
        return false;
    }

    // Check if more votes for than against
    proposal.votes_for > proposal.votes_against
}

/// Get all proposals (paginated)
pub fn get_all_proposals(env: &Env, offset: u32, limit: u32) -> Vec<Proposal> {
    let config = super::get_governance_config(env);
    let mut result: Vec<Proposal> = Vec::new(env);

    let start = offset + 1;
    let end = if offset + limit > config.proposal_count as u32 {
        config.proposal_count as u32
    } else {
        offset + limit
    };

    for i in start..=end {
        if let Some(proposal) = get_proposal(env, i as u64) {
            result.push_back(proposal);
        }
    }

    result
}

/// Get proposals by proposer
pub fn get_proposals_by_proposer(env: &Env, proposer: &Address) -> Vec<Proposal> {
    let config = super::get_governance_config(env);
    let mut result: Vec<Proposal> = Vec::new(env);

    for i in 1..=config.proposal_count {
        if let Some(proposal) = get_proposal(env, i) {
            if proposal.proposer == *proposer {
                result.push_back(proposal);
            }
        }
    }

    result
}

/// Get active proposals
pub fn get_active_proposals(env: &Env) -> Vec<Proposal> {
    let config = super::get_governance_config(env);
    let mut result: Vec<Proposal> = Vec::new(env);

    for i in 1..=config.proposal_count {
        if let Some(proposal) = get_proposal(env, i) {
            if is_proposal_active(env, proposal.id) {
                result.push_back(proposal);
            }
        }
    }

    result
}
