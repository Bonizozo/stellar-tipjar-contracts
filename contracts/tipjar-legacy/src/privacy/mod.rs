pub mod commitment;
pub mod contract_interface;
pub mod encrypted_operations;
pub mod homomorphic;
pub mod key_management;
pub mod zk_proof;

use soroban_sdk::{contracttype, Address, BytesN};

/// A privacy-preserving tip commitment.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateTip {
    /// Commitment hash: H(creator || amount || blinding_factor).
    pub commitment: BytesN<32>,
    /// Nullifier: prevents double-spend of the same commitment.
    pub nullifier: BytesN<32>,
}

/// Revealed commitment data for withdrawal.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitmentOpening {
    /// Creator address.
    pub creator: Address,
    /// Tip amount.
    pub amount: i128,
    /// Blinding factor used in commitment.
    pub blinding_factor: BytesN<32>,
}
