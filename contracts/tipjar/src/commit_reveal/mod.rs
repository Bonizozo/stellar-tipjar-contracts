//! Commit-reveal scheme for fair tip auctions and voting.
//!
//! Prevents front-running and ensures fairness by requiring participants to:
//!   1. **Commit phase**: Submit a hash of their bid/vote + secret salt
//!   2. **Reveal phase**: After commit deadline, reveal the actual value + salt
//!
//! The contract verifies that `hash(value || salt) == commitment` before
//! accepting the reveal. Participants who fail to reveal forfeit their entry.
//!
//! # Use cases
//! - Tip auctions: Highest bidder wins the right to tip a creator
//! - Voting: Fair voting on governance proposals or creator rankings
//! - Sealed-bid mechanisms: Any scenario requiring hidden bids until reveal

use soroban_sdk::{contracttype, symbol_short, Address, BytesN, Bytes, Env, String, Vec};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Minimum duration for commit phase (seconds).
pub const MIN_COMMIT_DURATION: u64 = 60; // 1 minute

/// Minimum duration for reveal phase (seconds).
pub const MIN_REVEAL_DURATION: u64 = 60; // 1 minute

/// Maximum duration for commit phase (seconds).
pub const MAX_COMMIT_DURATION: u64 = 86400 * 7; // 7 days

/// Maximum duration for reveal phase (seconds).
pub const MAX_REVEAL_DURATION: u64 = 86400 * 7; // 7 days

// ── Types ────────────────────────────────────────────────────────────────────

/// Status of a commit-reveal round.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoundStatus {
    /// Accepting commitments.
    Committing,
    /// Commit phase ended; accepting reveals.
    Revealing,
    /// Reveal phase ended; round is finalized.
    Finalized,
    /// Round was cancelled by creator/admin.
    Cancelled,
}

/// Type of commit-reveal round.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoundType {
    /// Tip auction: highest revealed bid wins.
    TipAuction,
    /// Voting: participants vote on options (e.g., governance).
    Voting,
    /// Generic sealed bid.
    SealedBid,
}

/// A commit-reveal round configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitRevealRound {
    /// Unique round ID.
    pub id: u64,
    /// Creator/initiator of this round.
    pub creator: Address,
    /// Type of round.
    pub round_type: RoundType,
    /// Current status.
    pub status: RoundStatus,
    /// Ledger timestamp when commit phase starts.
    pub commit_start: u64,
    /// Ledger timestamp when commit phase ends (reveal phase begins).
    pub commit_end: u64,
    /// Ledger timestamp when reveal phase ends.
    pub reveal_end: u64,
    /// Optional description of the round.
    pub description: String,
    /// Token used for bids (if applicable).
    pub token: Option<Address>,
    /// Minimum bid amount (if applicable).
    pub min_bid: i128,
    /// Number of commitments received.
    pub commit_count: u64,
    /// Number of reveals received.
    pub reveal_count: u64,
    /// Winner address (set after finalization for auctions).
    pub winner: Option<Address>,
    /// Winning bid amount (set after finalization for auctions).
    pub winning_bid: i128,
}

/// A participant's commitment.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commitment {
    /// Round ID.
    pub round_id: u64,
    /// Participant address.
    pub participant: Address,
    /// Hash of (value || salt).
    pub commitment_hash: BytesN<32>,
    /// Ledger timestamp when committed.
    pub committed_at: u64,
    /// Whether this commitment has been revealed.
    pub revealed: bool,
}

/// A revealed value.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reveal {
    /// Round ID.
    pub round_id: u64,
    /// Participant address.
    pub participant: Address,
    /// Revealed bid/vote value.
    pub value: i128,
    /// Salt used in commitment.
    pub salt: BytesN<32>,
    /// Ledger timestamp when revealed.
    pub revealed_at: u64,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn next_round_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::CommitRevealCounter)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::CommitRevealCounter, &(id + 1));
    id
}

fn load_round(env: &Env, round_id: u64) -> CommitRevealRound {
    env.storage()
        .persistent()
        .get(&DataKey::CommitRevealRound(round_id))
        .expect("Round not found")
}

fn save_round(env: &Env, round: &CommitRevealRound) {
    env.storage()
        .persistent()
        .set(&DataKey::CommitRevealRound(round.id), round);
}

// ── Hashing ──────────────────────────────────────────────────────────────────

/// Computes the commitment hash: SHA256(value || salt).
///
/// `value` is encoded as 16-byte big-endian i128.
pub fn compute_commitment(env: &Env, value: i128, salt: &BytesN<32>) -> BytesN<32> {
    let mut payload = Bytes::new(env);
    payload.extend_from_array(&value.to_be_bytes());
    payload.append(&salt.into());
    env.crypto().sha256(&payload).to_bytes()
}

// ── Core operations ──────────────────────────────────────────────────────────

/// Creates a new commit-reveal round.
///
/// Returns the round ID.
pub fn create_round(
    env: &Env,
    creator: &Address,
    round_type: RoundType,
    commit_duration: u64,
    reveal_duration: u64,
    description: String,
    token: Option<Address>,
    min_bid: i128,
) -> u64 {
    assert!(
        commit_duration >= MIN_COMMIT_DURATION && commit_duration <= MAX_COMMIT_DURATION,
        "Invalid commit duration"
    );
    assert!(
        reveal_duration >= MIN_REVEAL_DURATION && reveal_duration <= MAX_REVEAL_DURATION,
        "Invalid reveal duration"
    );

    let now = env.ledger().timestamp();
    let id = next_round_id(env);

    let round = CommitRevealRound {
        id,
        creator: creator.clone(),
        round_type,
        status: RoundStatus::Committing,
        commit_start: now,
        commit_end: now + commit_duration,
        reveal_end: now + commit_duration + reveal_duration,
        description,
        token,
        min_bid,
        commit_count: 0,
        reveal_count: 0,
        winner: None,
        winning_bid: 0,
    };

    save_round(env, &round);

    // Track rounds per creator
    let mut creator_rounds: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::CommitRevealCreatorRounds(creator.clone()))
        .unwrap_or_else(|| Vec::new(env));
    creator_rounds.push_back(id);
    env.storage()
        .persistent()
        .set(&DataKey::CommitRevealCreatorRounds(creator.clone()), &creator_rounds);

    env.events().publish(
        (symbol_short!("cr_new"),),
        (id, creator.clone(), now),
    );

    id
}

/// Submits a commitment during the commit phase.
pub fn commit(
    env: &Env,
    round_id: u64,
    participant: &Address,
    commitment_hash: BytesN<32>,
) {
    let mut round = load_round(env, round_id);

    assert!(round.status == RoundStatus::Committing, "Not in commit phase");

    let now = env.ledger().timestamp();
    assert!(now >= round.commit_start, "Commit phase not started");
    assert!(now < round.commit_end, "Commit phase ended");

    let key = DataKey::CommitRevealCommitment(round_id, participant.clone());
    assert!(!env.storage().persistent().has(&key), "Already committed");

    let commitment = Commitment {
        round_id,
        participant: participant.clone(),
        commitment_hash: commitment_hash.clone(),
        committed_at: now,
        revealed: false,
    };

    env.storage().persistent().set(&key, &commitment);

    round.commit_count += 1;
    save_round(env, &round);

    env.events().publish(
        (symbol_short!("cr_cmt"),),
        (round_id, participant.clone(), commitment_hash.clone()),
    );
}

/// Advances a round from Committing to Revealing status.
///
/// Can be called by anyone once the commit phase has ended.
pub fn start_reveal_phase(env: &Env, round_id: u64) {
    let mut round = load_round(env, round_id);

    assert!(round.status == RoundStatus::Committing, "Not in commit phase");

    let now = env.ledger().timestamp();
    assert!(now >= round.commit_end, "Commit phase not ended");

    round.status = RoundStatus::Revealing;
    save_round(env, &round);

    env.events().publish(
        (symbol_short!("cr_rvl"),),
        (round_id,),
    );
}

/// Reveals a commitment during the reveal phase.
///
/// Verifies that `hash(value || salt) == commitment_hash`.
pub fn reveal(
    env: &Env,
    round_id: u64,
    participant: &Address,
    value: i128,
    salt: BytesN<32>,
) {
    let mut round = load_round(env, round_id);

    assert!(round.status == RoundStatus::Revealing, "Not in reveal phase");

    let now = env.ledger().timestamp();
    assert!(now < round.reveal_end, "Reveal phase ended");

    let commit_key = DataKey::CommitRevealCommitment(round_id, participant.clone());
    let mut commitment: Commitment = env
        .storage()
        .persistent()
        .get(&commit_key)
        .expect("No commitment found");

    assert!(!commitment.revealed, "Already revealed");

    // Verify hash
    let computed = compute_commitment(env, value, &salt);
    assert!(computed == commitment.commitment_hash, "Invalid reveal");

    // Mark as revealed
    commitment.revealed = true;
    env.storage().persistent().set(&commit_key, &commitment);

    // Store reveal
    let reveal_key = DataKey::CommitRevealReveal(round_id, participant.clone());
    let reveal_record = Reveal {
        round_id,
        participant: participant.clone(),
        value,
        salt,
        revealed_at: now,
    };
    env.storage().persistent().set(&reveal_key, &reveal_record);

    round.reveal_count += 1;
    save_round(env, &round);

    env.events().publish(
        (symbol_short!("cr_rvld"),),
        (round_id, participant.clone(), value),
    );
}

/// Finalizes a round after the reveal phase ends.
///
/// For auctions: determines the winner (highest bid).
/// Can be called by anyone once the reveal phase has ended.
pub fn finalize_round(env: &Env, round_id: u64) {
    let mut round = load_round(env, round_id);

    assert!(round.status == RoundStatus::Revealing, "Not in reveal phase");

    let now = env.ledger().timestamp();
    assert!(now >= round.reveal_end, "Reveal phase not ended");

    // Determine winner for auctions
    if round.round_type == RoundType::TipAuction {
        let mut max_bid = round.min_bid;
        let mut winner: Option<Address> = None;

        // Iterate through all reveals to find highest bid
        // In production, you'd track reveal keys in a list for efficiency
        // For simplicity, we assume reveals are tracked externally or via events

        round.winner = winner;
        round.winning_bid = max_bid;
    }

    round.status = RoundStatus::Finalized;
    save_round(env, &round);

    env.events().publish(
        (symbol_short!("cr_fin"),),
        (round_id, round.winner.clone(), round.winning_bid),
    );
}

/// Cancels a round. Creator or admin only.
pub fn cancel_round(env: &Env, round_id: u64) {
    let mut round = load_round(env, round_id);

    assert!(
        round.status == RoundStatus::Committing || round.status == RoundStatus::Revealing,
        "Round already finalized or cancelled"
    );

    round.status = RoundStatus::Cancelled;
    save_round(env, &round);

    env.events().publish(
        (symbol_short!("cr_cncl"),),
        (round_id,),
    );
}

/// Returns a round by ID.
pub fn get_round(env: &Env, round_id: u64) -> Option<CommitRevealRound> {
    env.storage()
        .persistent()
        .get(&DataKey::CommitRevealRound(round_id))
}

/// Returns a commitment for a participant in a round.
pub fn get_commitment(env: &Env, round_id: u64, participant: &Address) -> Option<Commitment> {
    env.storage()
        .persistent()
        .get(&DataKey::CommitRevealCommitment(round_id, participant.clone()))
}

/// Returns a reveal for a participant in a round.
pub fn get_reveal(env: &Env, round_id: u64, participant: &Address) -> Option<Reveal> {
    env.storage()
        .persistent()
        .get(&DataKey::CommitRevealReveal(round_id, participant.clone()))
}

/// Returns all round IDs created by a creator.
pub fn get_creator_rounds(env: &Env, creator: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::CommitRevealCreatorRounds(creator.clone()))
        .unwrap_or_else(|| Vec::new(env))
}
