pub mod batch;
pub mod fraud_proof;

use soroban_sdk::{contracttype, Address, BytesN};

/// Duration of the challenge window in ledger seconds (7 days).
pub const CHALLENGE_PERIOD: u64 = 7 * 24 * 3600;

/// Status of a rollup batch.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BatchStatus {
    /// Submitted, within challenge period — not yet finalized.
    Pending,
    /// Challenge period elapsed with no valid fraud proof — credits applied.
    Finalized,
    /// A valid fraud proof was accepted; batch is rejected.
    Challenged,
}

/// A batch of tips submitted to the rollup by the sequencer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollupBatch {
    pub batch_id: u64,
    pub sequencer: Address,
    pub state_root: BytesN<32>,
    pub creator: Address,
    pub token: Address,
    pub total_amount: i128,
    pub tip_count: u32,
    pub submitted_at: u64,
    pub status: BatchStatus,
}

/// A fraud proof submitted by a challenger.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FraudProof {
    pub batch_id: u64,
    pub challenger: Address,
    pub claimed_root: BytesN<32>,
    pub submitted_at: u64,
}

/// Rollup summary state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollupState {
    pub enabled: bool,
    pub sequencer: Address,
    pub challenge_period: u64,
    pub pending_batches: u64,
    pub finalized_batches: u64,
    pub challenged_batches: u64,
}
