//! Governance System with Voting Mechanism
//!
//! This module provides on-chain governance functionality.

pub mod proposals;
pub mod voting;
pub mod timelock;

use soroban_sdk::{contracttype, Address, Env, String, Vec};

/// Proposal structure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u64,
    pub proposer: Address,
    pub description: String,
    pub actions: Vec<ProposalAction>,
    pub start_time: u64,
    pub end_time: u64,
    pub votes_for: i128,
    pub votes_against: i128,
    pub executed: bool,
    pub canceled: bool,
}

/// Proposal action types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalAction {
    UpdateFee { new_fee_bps: u32 },
    AddToken { token: Address },
    RemoveToken { token: Address },
    UpdatePauseStatus { paused: bool },
    UpdateTimelock { new_timelock: u64 },
    UpdateQuorum { new_quorum: i128 },
    UpdateProposalThreshold { new_threshold: i128 },
    UpdateVotingPeriod { new_period: u64 },
    UpdateVotingDelay { new_delay: u64 },
    Custom { data: String },
}

/// Vote choice
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoteChoice {
    For,
    Against,
    Abstain,
}

/// Vote record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vote {
    pub voter: Address,
    pub proposal_id: u64,
    pub choice: VoteChoice,
    pub voting_power: i128,
    pub timestamp: u64,
}

/// Governance configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceConfig {
    pub proposal_threshold: i128, // Minimum tokens to create proposal
    pub quorum: i128,             // Minimum votes for proposal to pass
    pub voting_period: u64,       // Duration of voting period in seconds
    pub voting_delay: u64,        // Delay before voting starts
    pub timelock_period: u64,     // Delay after voting before execution
    pub proposal_count: u64,
}

/// Storage keys for governance
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Proposal(u64),
    ProposalCounter,
    Vote(u64, Address),
    VoterVotes(u64),
    GovernanceConfig,
    ProposalThreshold,
    Quorum,
    VotingPeriod,
    VotingDelay,
    TimelockPeriod,
}

/// Default governance constants
pub const DEFAULT_PROPOSAL_THRESHOLD: i128 = 1000_000_000; // 1000 tokens
pub const DEFAULT_QUORUM: i128 = 10000_000_000; // 10000 tokens
pub const DEFAULT_VOTING_PERIOD: u64 = 604800; // 7 days
pub const DEFAULT_VOTING_DELAY: u64 = 86400; // 1 day
pub const DEFAULT_TIMELOCK_PERIOD: u64 = 172800; // 2 days

/// Initialize governance configuration
pub fn init_governance(env: &Env) {
    let config = GovernanceConfig {
        proposal_threshold: DEFAULT_PROPOSAL_THRESHOLD,
        quorum: DEFAULT_QUORUM,
        voting_period: DEFAULT_VOTING_PERIOD,
        voting_delay: DEFAULT_VOTING_DELAY,
        timelock_period: DEFAULT_TIMELOCK_PERIOD,
        proposal_count: 0,
    };

    env.storage()
        .persistent()
        .set(&DataKey::GovernanceConfig, &config);
}

/// Get governance configuration
pub fn get_governance_config(env: &Env) -> GovernanceConfig {
    env.storage()
        .persistent()
        .get(&DataKey::GovernanceConfig)
        .expect("Governance not initialized")
}

/// Update governance configuration
pub fn update_governance_config(env: &Env, config: &GovernanceConfig) {
    env.storage()
        .persistent()
        .set(&DataKey::GovernanceConfig, config);
}

/// Get next proposal ID
pub fn get_next_proposal_id(env: &Env) -> u64 {
    let config = get_governance_config(env);
    config.proposal_count + 1
}

/// Increment proposal count
pub fn increment_proposal_count(env: &Env) {
    let mut config = get_governance_config(env);
    config.proposal_count += 1;
    update_governance_config(env, &config);
}
