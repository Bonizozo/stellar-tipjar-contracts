//! Tip aggregation protocol.
//!
//! Batches multiple small tips into a single settlement transaction for gas
//! efficiency. The flow is:
//!
//! 1. **Queue** — tippers call `queue_tip` to add a pending tip to an
//!    aggregation batch keyed by `(token, creator)`. Tokens are transferred
//!    into the contract immediately and held in escrow.
//! 2. **Settle** — anyone (typically a keeper or the creator) calls
//!    `settle_batch` once the batch reaches the optimal size or the time
//!    window expires. The contract credits the creator's balance in one write.
//! 3. **Cancel** — a tipper may cancel their queued tip before settlement and
//!    receive a full refund.
//!
//! ## Optimal batch size
//! The optimal batch size is computed as the smallest batch that amortises the
//! fixed per-transaction overhead below a configurable threshold. The default
//! target is `OPTIMAL_BATCH_SIZE` tips; batches are also auto-settled when the
//! time window `BATCH_WINDOW_SECONDS` elapses.

pub mod batch;
pub mod queue;
pub mod settlement;

use soroban_sdk::{contracttype, Address};

// ── Constants ────────────────────────────────────────────────────────────────

/// Default number of tips that triggers an optimal settlement.
pub const OPTIMAL_BATCH_SIZE: u32 = 10;

/// Minimum batch size before manual settlement is allowed (prevents
/// premature settlement of tiny batches).
pub const MIN_BATCH_SIZE: u32 = 2;

/// Maximum tips that can be queued in a single batch.
pub const MAX_BATCH_SIZE: u32 = 50;

/// Time window in seconds after which a batch can be settled regardless of
/// size (prevents tips being stuck indefinitely).
pub const BATCH_WINDOW_SECONDS: u64 = 3_600; // 1 hour

/// Aggregation fee in basis points charged on settlement (0.1%).
/// Incentivises keepers to call `settle_batch`.
pub const AGGREGATION_FEE_BPS: u32 = 10;

// ── Data types ───────────────────────────────────────────────────────────────

/// Status of an aggregation batch.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatchStatus {
    /// Batch is open and accepting new tips.
    Open,
    /// Batch has been settled; creator balance credited.
    Settled,
    /// Batch was cancelled (all tips refunded).
    Cancelled,
}

/// An aggregation batch for a specific `(token, creator)` pair.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AggregationBatch {
    /// Unique batch ID.
    pub id: u64,
    /// Token being aggregated.
    pub token: Address,
    /// Creator who will receive the aggregated tips.
    pub creator: Address,
    /// Total amount queued in this batch (sum of all pending tips).
    pub total_queued: i128,
    /// Number of individual tips queued.
    pub tip_count: u32,
    /// Current batch status.
    pub status: BatchStatus,
    /// Timestamp when the batch was opened.
    pub opened_at: u64,
    /// Timestamp when the batch was settled or cancelled.
    pub closed_at: Option<u64>,
    /// Aggregation fee collected on settlement.
    pub fee_collected: i128,
}

/// A single queued tip within an aggregation batch.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedTip {
    /// Batch this tip belongs to.
    pub batch_id: u64,
    /// Index within the batch (0-based).
    pub index: u32,
    /// Tipper address.
    pub tipper: Address,
    /// Tip amount (gross, before fee).
    pub amount: i128,
    /// Timestamp when the tip was queued.
    pub queued_at: u64,
    /// Whether this tip has been refunded (cancelled).
    pub refunded: bool,
}

/// Summary returned after a batch settlement.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementResult {
    /// Batch ID that was settled.
    pub batch_id: u64,
    /// Number of tips included.
    pub tip_count: u32,
    /// Gross total transferred to creator (after fee).
    pub creator_amount: i128,
    /// Aggregation fee collected.
    pub fee_amount: i128,
    /// Timestamp of settlement.
    pub settled_at: u64,
}

/// Configuration for the aggregation protocol.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AggregationConfig {
    /// Target batch size for optimal settlement.
    pub optimal_batch_size: u32,
    /// Minimum tips before manual settlement is allowed.
    pub min_batch_size: u32,
    /// Maximum tips per batch.
    pub max_batch_size: u32,
    /// Seconds after which a batch can be force-settled.
    pub batch_window_seconds: u64,
    /// Aggregation fee in basis points.
    pub fee_bps: u32,
    /// Whether the aggregation protocol is enabled.
    pub enabled: bool,
}

impl AggregationConfig {
    /// Returns the default configuration.
    pub fn default() -> Self {
        AggregationConfig {
            optimal_batch_size: OPTIMAL_BATCH_SIZE,
            min_batch_size: MIN_BATCH_SIZE,
            max_batch_size: MAX_BATCH_SIZE,
            batch_window_seconds: BATCH_WINDOW_SECONDS,
            fee_bps: AGGREGATION_FEE_BPS,
            enabled: true,
        }
    }
}
