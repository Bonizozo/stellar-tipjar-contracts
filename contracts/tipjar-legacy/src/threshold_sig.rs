/// Threshold signature scheme for multi-party tip authorization.
///
/// A `ThresholdPolicy` defines a set of authorised signers and the minimum
/// number (`threshold`) required to approve a tip.  A `ThresholdTip` is a
/// pending tip that collects partial authorizations; once `threshold` unique
/// signers have submitted, the tip is marked ready for execution.
use soroban_sdk::{contracttype, Address, Env, Vec};

/// A named policy that governs threshold-authorized tips.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThresholdPolicy {
    /// Unique policy identifier.
    pub policy_id: u64,
    /// Address that created (and can update) this policy.
    pub owner: Address,
    /// Ordered list of authorised signers.
    pub signers: Vec<Address>,
    /// Minimum number of signers required to approve a tip.
    pub threshold: u32,
}

/// Status of a pending threshold tip.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThresholdTipStatus {
    /// Collecting partial signatures.
    Pending,
    /// Threshold reached; tip is ready to execute.
    Approved,
    /// Tip was executed (tokens transferred).
    Executed,
    /// Tip was cancelled by the proposer.
    Cancelled,
}

/// A pending tip awaiting threshold authorization.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThresholdTip {
    /// Unique tip identifier.
    pub tip_id: u64,
    /// Policy governing this tip.
    pub policy_id: u64,
    /// Address proposing the tip.
    pub proposer: Address,
    /// Tip recipient.
    pub creator: Address,
    /// Token to transfer.
    pub token: Address,
    /// Amount to transfer.
    pub amount: i128,
    /// Signers who have submitted a partial authorization.
    pub approvals: Vec<Address>,
    /// Current status.
    pub status: ThresholdTipStatus,
    /// Ledger timestamp when proposed.
    pub created_at: u64,
}

/// Returns true if `addr` is in `signers`.
pub fn is_signer(signers: &Vec<Address>, addr: &Address) -> bool {
    signers.contains(addr)
}

/// Returns true if the tip has reached the required threshold.
pub fn is_approved(tip: &ThresholdTip, threshold: u32) -> bool {
    tip.approvals.len() >= threshold
}
