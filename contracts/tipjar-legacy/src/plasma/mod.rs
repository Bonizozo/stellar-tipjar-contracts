//! Tip Plasma Chains
//!
//! Implements a Plasma chain for high-throughput tip processing.
//! The operator commits periodic block roots to the main chain.
//! Users can exit their funds via Merkle proofs, and anyone can
//! challenge invalid exits during the challenge window.

pub mod block;
pub mod exit;
pub mod challenge;

use soroban_sdk::{contracttype, Address, BytesN};

/// Duration of the exit challenge window in ledger seconds (7 days).
pub const EXIT_CHALLENGE_PERIOD: u64 = 7 * 24 * 3600;

/// Maximum number of tips that can be included in a single Plasma block.
pub const MAX_TIPS_PER_BLOCK: u32 = 1_000;

/// Status of a Plasma block commitment.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlasmaBlockStatus {
    /// Block has been committed but not yet finalized.
    Committed,
    /// Block is finalized — exits from it are valid.
    Finalized,
    /// Block was invalidated by a successful challenge.
    Invalidated,
}

/// Status of a Plasma exit request.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExitStatus {
    /// Exit is pending — within the challenge window.
    Pending,
    /// Exit was processed and funds released.
    Processed,
    /// Exit was challenged and cancelled.
    Challenged,
}

/// A Plasma block commitment anchored on the main chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlasmaBlock {
    /// Sequential block number.
    pub block_number: u64,
    /// Merkle root of all tip transactions in this block.
    pub tx_root: BytesN<32>,
    /// Operator who submitted this block.
    pub operator: Address,
    /// Total tip volume included in this block.
    pub total_volume: i128,
    /// Number of tip transactions in this block.
    pub tip_count: u32,
    /// Ledger timestamp when this block was committed.
    pub committed_at: u64,
    /// Current lifecycle status.
    pub status: PlasmaBlockStatus,
}

/// A user's exit request from the Plasma chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlasmaExit {
    /// Unique exit ID.
    pub exit_id: u64,
    /// Block number the exit references.
    pub block_number: u64,
    /// Address requesting the exit.
    pub exitor: Address,
    /// Token being exited.
    pub token: Address,
    /// Amount to be released on exit.
    pub amount: i128,
    /// Merkle proof leaf hash (hash of the tip transaction).
    pub tx_hash: BytesN<32>,
    /// Merkle proof path (sibling hashes from leaf to root).
    pub proof: soroban_sdk::Vec<BytesN<32>>,
    /// Ledger timestamp when the exit was initiated.
    pub initiated_at: u64,
    /// Current exit status.
    pub status: ExitStatus,
}

/// A challenge against an invalid exit.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExitChallenge {
    /// Exit ID being challenged.
    pub exit_id: u64,
    /// Address submitting the challenge.
    pub challenger: Address,
    /// Proof that the exit transaction was already spent.
    pub spend_tx_hash: BytesN<32>,
    /// Ledger timestamp when the challenge was submitted.
    pub submitted_at: u64,
}

/// Plasma chain state summary.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlasmaState {
    /// Whether the Plasma feature is enabled.
    pub enabled: bool,
    /// Authorized operator address.
    pub operator: Address,
    /// Latest committed block number.
    pub latest_block: u64,
    /// Total blocks committed.
    pub total_blocks: u64,
    /// Total exits processed.
    pub total_exits: u64,
    /// Total volume processed through Plasma.
    pub total_volume: i128,
}

/// Storage keys scoped to the Plasma module.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlasmaKey {
    /// Plasma feature enabled flag.
    Enabled,
    /// Authorized Plasma operator.
    Operator,
    /// Plasma block by block number.
    Block(u64),
    /// Latest committed block number.
    LatestBlock,
    /// Global block counter.
    BlockCounter,
    /// Exit request by exit ID.
    Exit(u64),
    /// Global exit ID counter.
    ExitCounter,
    /// Challenge for an exit.
    Challenge(u64),
    /// Pending exits for an address (list of exit IDs).
    UserExits(Address),
    /// Total volume finalized per creator per token.
    FinalizedVolume(Address, Address),
}
