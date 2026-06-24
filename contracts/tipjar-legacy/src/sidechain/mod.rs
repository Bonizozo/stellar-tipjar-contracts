pub mod checkpoint;
pub mod state;

use soroban_sdk::{contracttype, Address, BytesN};

/// Sidechain-specific storage keys.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SidechainKey {
    /// Sidechain operator (authorized to submit checkpoints).
    Operator,
    /// Whether the sidechain feature is enabled.
    Enabled,
    /// Checkpoint record keyed by sequence number.
    Checkpoint(u64),
    /// Latest finalized checkpoint sequence number.
    LatestCheckpoint,
    /// Pending tip batch keyed by batch ID.
    PendingBatch(u64),
    /// Global batch counter.
    BatchCounter,
    /// Finalized tip total per creator per token.
    FinalizedTotal(Address, Address),
}

/// A batch of tips aggregated on the sidechain, pending finalization.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipBatch {
    pub batch_id: u64,
    pub creator: Address,
    pub token: Address,
    pub total_amount: i128,
    pub tip_count: u32,
    pub checkpoint_seq: u64,
    pub finalized: bool,
}

/// A checkpoint submitted by the sidechain operator to anchor sidechain state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Checkpoint {
    pub seq: u64,
    pub state_root: BytesN<32>,
    pub tip_count: u32,
    pub total_volume: i128,
    pub submitted_at: u64,
    pub finalized: bool,
}

/// Current sidechain state summary.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SidechainState {
    pub enabled: bool,
    pub latest_checkpoint: u64,
    pub total_checkpoints: u64,
    pub total_finalized_volume: i128,
}
