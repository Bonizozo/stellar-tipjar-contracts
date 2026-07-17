//! Condition type definitions for conditional tip execution.

use soroban_sdk::{contracttype, Address, BytesN};

/// A single condition that must pass before a tip is executed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Condition {
    /// Condition that always returns true.
    Always,
    /// Require current ledger sequence to be at least this value.
    MinLedger(u32),
    /// Require current ledger sequence to be at most this value.
    MaxLedger(u32),
    /// Require current ledger timestamp to be at least this value.
    TimeAfter(u64),
    /// Require current ledger timestamp to be at most this value.
    TimeBefore(u64),
    /// Require token balance for account to be at least `min_balance`.
    TokenBalanceAtLeast(Address, Address, i128),
    /// Require an off-chain oracle approval keyed by condition ID.
    OffchainApproved(BytesN<32>),
}
