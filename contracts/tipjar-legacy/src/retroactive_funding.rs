//! Retroactive Public Goods Funding
//!
//! Rewards past contributions to the ecosystem:
//! - Admins create funding rounds with a token pool and evaluation criteria.
//! - Voters cast weighted votes for eligible creators based on impact metrics.
//! - After the voting window closes, rewards are distributed proportionally to
//!   vote-weighted impact scores.
//! - Impact metrics (total tips received, tip count) are read from existing
//!   contract storage to evaluate past contributions.

use soroban_sdk::{contracttype, panic_with_error, symbol_short, token, Address, Env, Vec};

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetroKey {
    /// Round record keyed by round_id.
    Round(u64),
    /// Global round counter.
    RoundCtr,
    /// Vote cast by a voter for a creator in a round: (round_id, voter, creator).
    Vote(u64, Address, Address),
    /// Aggregated vote weight for a creator in a round: (round_id, creator).
    CreatorVotes(u64, Address),
    /// List of creators nominated in a round.
    RoundCreators(u64),
    /// List of voters who have voted in a round.
    RoundVoters(u64),
    /// Whether a voter has already voted in a round: (round_id, voter).
    HasVoted(u64, Address),
    /// Reward claimed flag: (round_id, creator).
    Claimed(u64, Address),
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// Status of a retroactive funding round.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RetroRoundStatus {
    /// Accepting nominations and votes.
    Active,
    /// Voting closed; rewards can be claimed.
    Finalized,
    /// All rewards distributed.
    Distributed,
}

/// Evaluation criteria weights (basis points, must sum to 10 000).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvalCriteria {
    /// Weight given to total historical tips received (bps).
    pub tips_weight_bps: u32,
    /// Weight given to number of unique tips received (bps).
    pub tip_count_weight_bps: u32,
    /// Weight given to raw voter votes (bps).
    pub vote_weight_bps: u32,
}

/// A retroactive funding round.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetroRound {
    pub round_id: u64,
    pub admin: Address,
    pub token: Address,
    /// Total reward pool deposited by admin.
    pub reward_pool: i128,
    /// Voting opens at this timestamp.
    pub start_time: u64,
    /// Voting closes at this timestamp.
    pub end_time: u64,
    pub status: RetroRoundStatus,
    pub criteria: EvalCriteria,
    /// Total vote weight cast across all creators.
    pub total_votes: i128,
}

/// A single vote record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetroVote {
    pub voter: Address,
    pub creator: Address,
    pub round_id: u64,
    /// Vote weight (e.g. proportional to voter's own tip history).
    pub weight: i128,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RetroError {
    RoundNotFound = 700,
    RoundNotActive = 701,
    RoundNotFinalized = 702,
    AlreadyDistributed = 703,
    RoundEnded = 704,
    RoundNotEnded = 705,
    AlreadyVoted = 706,
    InvalidAmount = 707,
    Unauthorized = 708,
    CreatorNotNominated = 709,
    AlreadyClaimed = 710,
    NothingToClaim = 711,
    InvalidCriteria = 712,
}

// ── Integer square root (no floating point in Soroban) ───────────────────────

fn isqrt(n: i128) -> i128 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Creates a new retroactive funding round.
///
/// Transfers `reward_pool` tokens from `admin` into the contract.
/// `criteria` weights must sum to 10 000 bps.
///
/// Emits `("retro_new",)` with `(round_id, token, reward_pool, end_time)`.
pub fn create_round(
    env: &Env,
    admin: &Address,
    token: &Address,
    reward_pool: i128,
    start_time: u64,
    end_time: u64,
    criteria: EvalCriteria,
) -> u64 {
    admin.require_auth();

    if reward_pool <= 0 {
        panic_with_error!(env, RetroError::InvalidAmount);
    }
    if end_time <= start_time {
        panic_with_error!(env, RetroError::InvalidAmount);
    }
    let weight_sum =
        criteria.tips_weight_bps + criteria.tip_count_weight_bps + criteria.vote_weight_bps;
    if weight_sum != 10_000 {
        panic_with_error!(env, RetroError::InvalidCriteria);
    }

    let round_id: u64 = env
        .storage()
        .instance()
        .get(&crate::DataKey::Retro(RetroKey::RoundCtr))
        .unwrap_or(0u64);
    env.storage()
        .instance()
        .set(&crate::DataKey::Retro(RetroKey::RoundCtr), &(round_id + 1));

    let round = RetroRound {
        round_id,
        admin: admin.clone(),
        token: token.clone(),
        reward_pool,
        start_time,
        end_time,
        status: RetroRoundStatus::Active,
        criteria,
        total_votes: 0,
    };
    env.storage()
        .persistent()
        .set(&crate::DataKey::Retro(RetroKey::Round(round_id)), &round);

    token::Client::new(env, token).transfer(admin, &env.current_contract_address(), &reward_pool);

    env.events().publish(
        (symbol_short!("retro_new"),),
        (round_id, token.clone(), reward_pool, end_time),
    );
    round_id
}

/// Nominates a creator as eligible for rewards in a round.
///
/// Only the round admin may nominate. Nomination must happen before voting ends.
/// Emits `("retro_nom",)` with `(round_id, creator)`.
pub fn nominate_creator(env: &Env, admin: &Address, round_id: u64, creator: &Address) {
    admin.require_auth();

    let round: RetroRound = env
        .storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::Round(round_id)))
        .unwrap_or_else(|| panic_with_error!(env, RetroError::RoundNotFound));

    if round.admin != *admin {
        panic_with_error!(env, RetroError::Unauthorized);
    }
    if round.status != RetroRoundStatus::Active {
        panic_with_error!(env, RetroError::RoundNotActive);
    }
    if env.ledger().timestamp() > round.end_time {
        panic_with_error!(env, RetroError::RoundEnded);
    }

    let mut creators: Vec<Address> = env
        .storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::RoundCreators(round_id)))
        .unwrap_or_else(|| Vec::new(env));
    if !creators.contains(creator) {
        creators.push_back(creator.clone());
        env.storage().persistent().set(
            &crate::DataKey::Retro(RetroKey::RoundCreators(round_id)),
            &creators,
        );
    }

    env.events()
        .publish((symbol_short!("retro_nom"),), (round_id, creator.clone()));
}

/// Casts a vote for a nominated creator in an active round.
///
/// Each voter may vote once per round. Vote weight is derived from the voter's
/// own total tips sent (stored in `CreatorTotal` for the round token), giving
/// more influence to active ecosystem participants. Falls back to weight = 1
/// if the voter has no tip history.
///
/// Emits `("retro_vote",)` with `(round_id, voter, creator, weight)`.
pub fn cast_vote(env: &Env, voter: &Address, round_id: u64, creator: &Address) {
    voter.require_auth();

    let mut round: RetroRound = env
        .storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::Round(round_id)))
        .unwrap_or_else(|| panic_with_error!(env, RetroError::RoundNotFound));

    if round.status != RetroRoundStatus::Active {
        panic_with_error!(env, RetroError::RoundNotActive);
    }
    let now = env.ledger().timestamp();
    if now < round.start_time || now > round.end_time {
        panic_with_error!(env, RetroError::RoundEnded);
    }

    // Ensure creator is nominated.
    let creators: Vec<Address> = env
        .storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::RoundCreators(round_id)))
        .unwrap_or_else(|| Vec::new(env));
    if !creators.contains(creator) {
        panic_with_error!(env, RetroError::CreatorNotNominated);
    }

    // One vote per voter per round.
    let has_voted: bool = env
        .storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::HasVoted(
            round_id,
            voter.clone(),
        )))
        .unwrap_or(false);
    if has_voted {
        panic_with_error!(env, RetroError::AlreadyVoted);
    }

    // Vote weight = sqrt(voter's total tips sent in round token), min 1.
    let voter_tips: i128 = env
        .storage()
        .persistent()
        .get(&crate::DataKey::CreatorTotal(
            voter.clone(),
            round.token.clone(),
        ))
        .unwrap_or(0);
    let weight = isqrt(voter_tips).max(1);

    // Record vote.
    let vote = RetroVote {
        voter: voter.clone(),
        creator: creator.clone(),
        round_id,
        weight,
    };
    env.storage().persistent().set(
        &crate::DataKey::Retro(RetroKey::Vote(round_id, voter.clone(), creator.clone())),
        &vote,
    );
    env.storage().persistent().set(
        &crate::DataKey::Retro(RetroKey::HasVoted(round_id, voter.clone())),
        &true,
    );

    // Accumulate creator vote weight.
    let prev: i128 = env
        .storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::CreatorVotes(
            round_id,
            creator.clone(),
        )))
        .unwrap_or(0);
    env.storage().persistent().set(
        &crate::DataKey::Retro(RetroKey::CreatorVotes(round_id, creator.clone())),
        &(prev + weight),
    );

    round.total_votes += weight;
    env.storage()
        .persistent()
        .set(&crate::DataKey::Retro(RetroKey::Round(round_id)), &round);

    env.events().publish(
        (symbol_short!("retro_vot"),),
        (round_id, voter.clone(), creator.clone(), weight),
    );
}

/// Finalizes a round after the voting window closes.
///
/// Only the round admin may finalize. Emits `("retro_fin",)` with `(round_id, total_votes)`.
pub fn finalize_round(env: &Env, admin: &Address, round_id: u64) {
    admin.require_auth();

    let mut round: RetroRound = env
        .storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::Round(round_id)))
        .unwrap_or_else(|| panic_with_error!(env, RetroError::RoundNotFound));

    if round.admin != *admin {
        panic_with_error!(env, RetroError::Unauthorized);
    }
    if round.status != RetroRoundStatus::Active {
        panic_with_error!(env, RetroError::RoundNotActive);
    }
    if env.ledger().timestamp() <= round.end_time {
        panic_with_error!(env, RetroError::RoundNotEnded);
    }

    round.status = RetroRoundStatus::Finalized;
    env.storage()
        .persistent()
        .set(&crate::DataKey::Retro(RetroKey::Round(round_id)), &round);

    env.events()
        .publish((symbol_short!("retro_fin"),), (round_id, round.total_votes));
}

/// Computes a creator's impact score for a finalized round.
///
/// Score = (tips_total * tips_weight + tip_count * count_weight + votes * vote_weight) / 10_000
/// where tip_count is scaled by 1_000_000 to be comparable with token amounts.
pub fn compute_impact_score(env: &Env, round_id: u64, creator: &Address) -> i128 {
    let round: RetroRound = env
        .storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::Round(round_id)))
        .unwrap_or_else(|| panic_with_error!(env, RetroError::RoundNotFound));

    let total_tips: i128 = env
        .storage()
        .persistent()
        .get(&crate::DataKey::CreatorTotal(
            creator.clone(),
            round.token.clone(),
        ))
        .unwrap_or(0);

    let tip_count: i128 = env
        .storage()
        .persistent()
        .get(&crate::DataKey::TipCount(creator.clone()))
        .unwrap_or(0u64) as i128;

    let votes: i128 = env
        .storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::CreatorVotes(
            round_id,
            creator.clone(),
        )))
        .unwrap_or(0);

    let c = &round.criteria;
    (total_tips * c.tips_weight_bps as i128
        + tip_count * 1_000_000 * c.tip_count_weight_bps as i128
        + votes * 1_000_000 * c.vote_weight_bps as i128)
        / 10_000
}

/// Claims the retroactive reward for a creator in a finalized round.
///
/// Reward = reward_pool * creator_score / total_score_across_all_creators.
/// Emits `("retro_clm",)` with `(round_id, creator, amount)`.
pub fn claim_reward(env: &Env, creator: &Address, round_id: u64) -> i128 {
    creator.require_auth();

    let round: RetroRound = env
        .storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::Round(round_id)))
        .unwrap_or_else(|| panic_with_error!(env, RetroError::RoundNotFound));

    if round.status != RetroRoundStatus::Finalized {
        panic_with_error!(env, RetroError::RoundNotFinalized);
    }

    let claimed: bool = env
        .storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::Claimed(
            round_id,
            creator.clone(),
        )))
        .unwrap_or(false);
    if claimed {
        panic_with_error!(env, RetroError::AlreadyClaimed);
    }

    // Ensure creator is nominated.
    let creators: Vec<Address> = env
        .storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::RoundCreators(round_id)))
        .unwrap_or_else(|| Vec::new(env));
    if !creators.contains(creator) {
        panic_with_error!(env, RetroError::CreatorNotNominated);
    }

    // Compute total score across all nominated creators.
    let mut total_score: i128 = 0;
    for c in creators.iter() {
        total_score += compute_impact_score(env, round_id, &c);
    }

    let creator_score = compute_impact_score(env, round_id, creator);
    if creator_score == 0 || total_score == 0 {
        panic_with_error!(env, RetroError::NothingToClaim);
    }

    let reward = (round.reward_pool * creator_score) / total_score;
    if reward == 0 {
        panic_with_error!(env, RetroError::NothingToClaim);
    }

    env.storage().persistent().set(
        &crate::DataKey::Retro(RetroKey::Claimed(round_id, creator.clone())),
        &true,
    );

    token::Client::new(env, &round.token).transfer(
        &env.current_contract_address(),
        creator,
        &reward,
    );

    env.events().publish(
        (symbol_short!("retro_clm"),),
        (round_id, creator.clone(), reward),
    );
    reward
}

/// Returns a round by ID.
pub fn get_round(env: &Env, round_id: u64) -> RetroRound {
    env.storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::Round(round_id)))
        .unwrap_or_else(|| panic_with_error!(env, RetroError::RoundNotFound))
}

/// Returns the nominated creators for a round.
pub fn get_round_creators(env: &Env, round_id: u64) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::RoundCreators(round_id)))
        .unwrap_or_else(|| Vec::new(env))
}

/// Returns the aggregated vote weight for a creator in a round.
pub fn get_creator_votes(env: &Env, round_id: u64, creator: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::CreatorVotes(
            round_id,
            creator.clone(),
        )))
        .unwrap_or(0)
}

/// Returns whether a voter has already voted in a round.
pub fn has_voted(env: &Env, round_id: u64, voter: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&crate::DataKey::Retro(RetroKey::HasVoted(
            round_id,
            voter.clone(),
        )))
        .unwrap_or(false)
}
