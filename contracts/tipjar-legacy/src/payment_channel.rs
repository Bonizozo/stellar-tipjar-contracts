/// Bidirectional payment channel for instant tip settlements.
///
/// Lifecycle:
///   open_channel → update_channel_state (off-chain, many times) → cooperative_close
///                                                                 → dispute_close (unilateral)
use soroban_sdk::{contracttype, Address, Env};

/// Status of a payment channel.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChannelStatus {
    /// Channel is open and accepting state updates.
    Open,
    /// A unilateral close has been initiated; waiting for dispute window.
    Disputed,
    /// Channel is closed; funds have been distributed.
    Closed,
}

/// On-chain record for a payment channel between two parties.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentChannel {
    /// First party (channel opener).
    pub party_a: Address,
    /// Second party (counterparty).
    pub party_b: Address,
    /// Token used in this channel.
    pub token: Address,
    /// Total collateral locked in the channel (party_a + party_b deposits).
    pub total_deposit: i128,
    /// Latest agreed balance for party_a (party_b gets total_deposit - balance_a).
    pub balance_a: i128,
    /// Monotonically increasing state version; prevents replay of stale states.
    pub nonce: u64,
    /// Current channel status.
    pub status: ChannelStatus,
    /// Ledger timestamp when the channel was opened.
    pub opened_at: u64,
    /// When a dispute was initiated (0 if none).
    pub dispute_started_at: u64,
    /// Seconds the counterparty has to submit a newer state during a dispute.
    pub dispute_window: u64,
    /// Address that initiated the current dispute (if any).
    pub disputer: Option<Address>,
}

/// Opens a channel, transferring `deposit_a` from party_a and `deposit_b` from party_b.
pub fn open(
    env: &Env,
    party_a: &Address,
    party_b: &Address,
    token: &Address,
    deposit_a: i128,
    deposit_b: i128,
    dispute_window: u64,
) -> PaymentChannel {
    let total = deposit_a + deposit_b;
    PaymentChannel {
        party_a: party_a.clone(),
        party_b: party_b.clone(),
        token: token.clone(),
        total_deposit: total,
        balance_a: deposit_a, // initial split: each party owns their deposit
        nonce: 0,
        status: ChannelStatus::Open,
        opened_at: env.ledger().timestamp(),
        dispute_started_at: 0,
        dispute_window,
        disputer: None,
    }
}

/// Returns the balance owed to party_b given the current channel state.
pub fn balance_b(channel: &PaymentChannel) -> i128 {
    channel.total_deposit - channel.balance_a
}
