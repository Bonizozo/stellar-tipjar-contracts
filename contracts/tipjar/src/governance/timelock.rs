//! Timelock mechanism for governance

use super::{ProposalAction, DataKey};
use soroban_sdk::{Address, Env, String, Vec};

/// Execute a proposal after timelock
pub fn execute_proposal(env: &Env, executor: &Address, proposal_id: u64) {
    executor.require_auth();

    let proposal = super::proposals::get_proposal_or_panic(env, proposal_id);

    // Check proposal has passed
    if !super::proposals::has_proposal_passed(env, proposal_id) {
        panic!("Proposal has not passed");
    }

    // Check timelock has expired
    let now = env.ledger().timestamp();
    let config = super::get_governance_config(env);
    if now < proposal.end_time + config.timelock_period {
        panic!("Timelock period not expired");
    }

    // Check not already executed
    if proposal.executed {
        panic!("Proposal already executed");
    }

    // Execute actions
    for action in proposal.actions.iter() {
        execute_action(env, action);
    }

    // Mark as executed
    super::proposals::mark_proposal_executed(env, proposal_id);
}

/// Execute a single action
fn execute_action(env: &Env, action: &ProposalAction) {
    match action {
        ProposalAction::UpdateFee { new_fee_bps } => {
            // In production, this would update the AMM fee
            // For now, we'll just log the action
            env.events().publish(
                (soroban_sdk::symbol_short!("fee_upd"),),
                *new_fee_bps,
            );
        }
        ProposalAction::AddToken { token } => {
            // In production, this would add token to whitelist
            env.events().publish(
                (soroban_sdk::symbol_short!("token_add"),),
                token.clone(),
            );
        }
        ProposalAction::RemoveToken { token } => {
            // In production, this would remove token from whitelist
            env.events().publish(
                (soroban_sdk::symbol_short!("token_rm"),),
                token.clone(),
            );
        }
        ProposalAction::UpdatePauseStatus { paused } => {
            // In production, this would update pause status
            env.events().publish(
                (soroban_sdk::symbol_short!("pause_upd"),),
                *paused,
            );
        }
        ProposalAction::UpdateTimelock { new_timelock } => {
            // Update timelock in governance config
            let mut config = super::get_governance_config(env);
            config.timelock_period = *new_timelock;
            super::update_governance_config(env, &config);
        }
        ProposalAction::UpdateQuorum { new_quorum } => {
            // Update quorum in governance config
            let mut config = super::get_governance_config(env);
            config.quorum = *new_quorum;
            super::update_governance_config(env, &config);
        }
        ProposalAction::UpdateProposalThreshold { new_threshold } => {
            // Update proposal threshold in governance config
            let mut config = super::get_governance_config(env);
            config.proposal_threshold = *new_threshold;
            super::update_governance_config(env, &config);
        }
        ProposalAction::UpdateVotingPeriod { new_period } => {
            // Update voting period in governance config
            let mut config = super::get_governance_config(env);
            config.voting_period = *new_period;
            super::update_governance_config(env, &config);
        }
        ProposalAction::UpdateVotingDelay { new_delay } => {
            // Update voting delay in governance config
            let mut config = super::get_governance_config(env);
            config.voting_delay = *new_delay;
            super::update_governance_config(env, &config);
        }
        ProposalAction::Custom { data } => {
            // Custom action - just log it
            env.events().publish(
                (soroban_sdk::symbol_short!("custom"),),
                data.clone(),
            );
        }
    }
}

/// Cancel a proposal (only by proposer before voting starts)
pub fn cancel_proposal(env: &Env, proposer: &Address, proposal_id: u64) {
    proposer.require_auth();

    let proposal = super::proposals::get_proposal_or_panic(env, proposal_id);

    // Check proposer is the original proposer
    if proposal.proposer != *proposer {
        panic!("Only proposer can cancel");
    }

    // Check voting hasn't started
    let now = env.ledger().timestamp();
    if now >= proposal.start_time {
        panic!("Voting has already started");
    }

    // Check not already executed or canceled
    if proposal.executed || proposal.canceled {
        panic!("Proposal already executed or canceled");
    }

    super::proposals::mark_proposal_canceled(env, proposal_id);
}

/// Get time until timelock expires
pub fn get_timelock_remaining(env: &Env, proposal_id: u64) -> u64 {
    let proposal = super::proposals::get_proposal_or_panic(env, proposal_id);
    let config = super::get_governance_config(env);
    let now = env.ledger().timestamp();

    let timelock_end = proposal.end_time + config.timelock_period;
    if now >= timelock_end {
        0
    } else {
        timelock_end - now
    }
}

/// Check if proposal can be executed
pub fn can_execute(env: &Env, proposal_id: u64) -> bool {
    let proposal = super::proposals::get_proposal_or_panic(env, proposal_id);

    if proposal.executed || proposal.canceled {
        return false;
    }

    if !super::proposals::has_proposal_passed(env, proposal_id) {
        return false;
    }

    let now = env.ledger().timestamp();
    let config = super::get_governance_config(env);
    now >= proposal.end_time + config.timelock_period
}
