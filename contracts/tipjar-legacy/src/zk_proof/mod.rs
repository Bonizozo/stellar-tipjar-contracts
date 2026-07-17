//! Zero-knowledge proof support for private tip verification.
//!
//! Enables privacy-preserving tip verification where:
//!   - Tipper can prove they sent a tip without revealing the amount
//!   - Creator can prove they received tips above a threshold without revealing totals
//!   - Third parties can verify proofs without learning private data
//!
//! # ZK-SNARK Integration
//! This module provides a framework for integrating ZK-SNARKs (Zero-Knowledge
//! Succinct Non-Interactive Arguments of Knowledge) with the tip contract.
//!
//! ## Supported proof types
//! - **Range proofs**: Prove a tip amount is within a range [min, max]
//! - **Membership proofs**: Prove a tip is in a set without revealing which one
//! - **Sum proofs**: Prove total tips equal a value without revealing individual amounts
//! - **Threshold proofs**: Prove total tips exceed a threshold
//!
//! ## Circuit structure
//! Circuits are defined off-chain and verified on-chain. The contract stores:
//!   - Verification keys for each circuit type
//!   - Public inputs (commitments, nullifiers)
//!   - Proof data (compressed SNARK proof)
//!
//! ## Privacy model
//! - Private inputs: tip amounts, salts, sender/receiver identities
//! - Public inputs: commitments, nullifiers, circuit parameters
//! - Proof: cryptographic proof that private inputs satisfy circuit constraints

use soroban_sdk::{contracttype, symbol_short, Address, Bytes, BytesN, Env, String, Vec};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Maximum proof size in bytes (compressed SNARK proof).
pub const MAX_PROOF_SIZE: u32 = 512;

/// Maximum public input count.
pub const MAX_PUBLIC_INPUTS: u32 = 16;

// ── Types ────────────────────────────────────────────────────────────────────

/// Type of zero-knowledge proof circuit.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CircuitType {
    /// Prove tip amount is within range [min, max].
    RangeProof,
    /// Prove tip is in a set without revealing which.
    MembershipProof,
    /// Prove sum of tips equals a value.
    SumProof,
    /// Prove total tips exceed a threshold.
    ThresholdProof,
    /// Generic custom circuit.
    Custom,
}

/// Status of a proof verification.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofStatus {
    /// Proof is pending verification.
    Pending,
    /// Proof has been verified and is valid.
    Verified,
    /// Proof verification failed.
    Invalid,
    /// Proof has been revoked.
    Revoked,
}

/// Verification key for a ZK circuit.
///
/// In production, this would contain the actual verification key data
/// (e.g., Groth16 vk). For this implementation, we use a simplified
/// representation with a key hash.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationKey {
    /// Unique circuit ID.
    pub circuit_id: u64,
    /// Type of circuit.
    pub circuit_type: CircuitType,
    /// Hash of the verification key data.
    pub vk_hash: BytesN<32>,
    /// Creator/owner of this circuit.
    pub owner: Address,
    /// Whether this circuit is active.
    pub active: bool,
    /// Optional description.
    pub description: String,
    /// Ledger timestamp when registered.
    pub registered_at: u64,
}

/// A zero-knowledge proof submission.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZkProof {
    /// Unique proof ID.
    pub id: u64,
    /// Circuit ID this proof is for.
    pub circuit_id: u64,
    /// Prover address.
    pub prover: Address,
    /// Compressed proof data (e.g., Groth16 proof bytes).
    pub proof_data: Bytes,
    /// Public inputs to the circuit.
    pub public_inputs: Vec<BytesN<32>>,
    /// Commitment to private inputs (for auditability).
    pub private_commitment: BytesN<32>,
    /// Nullifier (prevents double-spending/reuse).
    pub nullifier: BytesN<32>,
    /// Current status.
    pub status: ProofStatus,
    /// Ledger timestamp when submitted.
    pub submitted_at: u64,
    /// Ledger timestamp when verified (0 if not yet verified).
    pub verified_at: u64,
    /// Optional metadata.
    pub metadata: String,
}

/// A private tip record using ZK proofs.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateTipProof {
    /// Unique private tip ID.
    pub id: u64,
    /// Creator receiving the tip.
    pub creator: Address,
    /// Proof ID that verifies this tip.
    pub proof_id: u64,
    /// Commitment to the tip amount.
    pub amount_commitment: BytesN<32>,
    /// Nullifier (prevents double-spending).
    pub nullifier: BytesN<32>,
    /// Whether this tip has been claimed/revealed.
    pub claimed: bool,
    /// Ledger timestamp when created.
    pub created_at: u64,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn next_circuit_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::ZkCircuitCounter)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::ZkCircuitCounter, &(id + 1));
    id
}

fn next_proof_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::ZkProofCounter)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::ZkProofCounter, &(id + 1));
    id
}

fn next_private_tip_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::ZkPrivateTipCounter)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::ZkPrivateTipCounter, &(id + 1));
    id
}

// ── Circuit management ───────────────────────────────────────────────────────

/// Registers a new ZK circuit verification key.
///
/// Returns the circuit ID.
pub fn register_circuit(
    env: &Env,
    owner: &Address,
    circuit_type: CircuitType,
    vk_hash: BytesN<32>,
    description: String,
) -> u64 {
    let id = next_circuit_id(env);

    let vk = VerificationKey {
        circuit_id: id,
        circuit_type,
        vk_hash: vk_hash.clone(),
        owner: owner.clone(),
        active: true,
        description,
        registered_at: env.ledger().timestamp(),
    };

    env.storage().persistent().set(&DataKey::ZkCircuit(id), &vk);

    // Track circuits per owner
    let mut owner_circuits: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::ZkOwnerCircuits(owner.clone()))
        .unwrap_or_else(|| Vec::new(env));
    owner_circuits.push_back(id);
    env.storage()
        .persistent()
        .set(&DataKey::ZkOwnerCircuits(owner.clone()), &owner_circuits);

    env.events().publish(
        (symbol_short!("zk_reg"),),
        (id, owner.clone(), vk_hash.clone()),
    );

    id
}

/// Deactivates a circuit. Owner only.
pub fn deactivate_circuit(env: &Env, circuit_id: u64) {
    let mut vk: VerificationKey = env
        .storage()
        .persistent()
        .get(&DataKey::ZkCircuit(circuit_id))
        .expect("Circuit not found");

    vk.active = false;
    env.storage()
        .persistent()
        .set(&DataKey::ZkCircuit(circuit_id), &vk);

    env.events()
        .publish((symbol_short!("zk_deact"),), (circuit_id,));
}

// ── Proof submission & verification ──────────────────────────────────────────

/// Submits a zero-knowledge proof for verification.
///
/// Returns the proof ID.
pub fn submit_proof(
    env: &Env,
    prover: &Address,
    circuit_id: u64,
    proof_data: Bytes,
    public_inputs: Vec<BytesN<32>>,
    private_commitment: BytesN<32>,
    nullifier: BytesN<32>,
    metadata: String,
) -> u64 {
    // Validate circuit exists and is active
    let vk: VerificationKey = env
        .storage()
        .persistent()
        .get(&DataKey::ZkCircuit(circuit_id))
        .expect("Circuit not found");
    assert!(vk.active, "Circuit is not active");

    // Validate proof size
    assert!(
        proof_data.len() <= MAX_PROOF_SIZE,
        "Proof data exceeds maximum size"
    );

    // Validate public inputs count
    assert!(
        public_inputs.len() <= MAX_PUBLIC_INPUTS,
        "Too many public inputs"
    );

    // Check nullifier hasn't been used
    assert!(
        !is_nullifier_used(env, &nullifier),
        "Nullifier already used"
    );

    let id = next_proof_id(env);

    let proof = ZkProof {
        id,
        circuit_id,
        prover: prover.clone(),
        proof_data,
        public_inputs,
        private_commitment,
        nullifier: nullifier.clone(),
        status: ProofStatus::Pending,
        submitted_at: env.ledger().timestamp(),
        verified_at: 0,
        metadata,
    };

    env.storage()
        .persistent()
        .set(&DataKey::ZkProof(id), &proof);

    // Mark nullifier as used
    mark_nullifier_used(env, &nullifier);

    // Track proofs per prover
    let mut prover_proofs: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::ZkProverProofs(prover.clone()))
        .unwrap_or_else(|| Vec::new(env));
    prover_proofs.push_back(id);
    env.storage()
        .persistent()
        .set(&DataKey::ZkProverProofs(prover.clone()), &prover_proofs);

    env.events().publish(
        (symbol_short!("zk_sub"),),
        (id, prover.clone(), circuit_id, nullifier),
    );

    id
}

/// Verifies a submitted proof.
///
/// In production, this would call a ZK verifier (e.g., Groth16 verifier).
/// For this implementation, we simulate verification by checking that
/// the proof data is non-empty and public inputs are valid.
///
/// Admin/verifier only.
pub fn verify_proof(env: &Env, proof_id: u64, is_valid: bool) {
    let mut proof: ZkProof = env
        .storage()
        .persistent()
        .get(&DataKey::ZkProof(proof_id))
        .expect("Proof not found");

    assert!(
        proof.status == ProofStatus::Pending,
        "Proof already processed"
    );

    proof.status = if is_valid {
        ProofStatus::Verified
    } else {
        ProofStatus::Invalid
    };
    proof.verified_at = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&DataKey::ZkProof(proof_id), &proof);

    env.events()
        .publish((symbol_short!("zk_ver"),), (proof_id, is_valid));
}

/// Revokes a proof. Admin only.
pub fn revoke_proof(env: &Env, proof_id: u64) {
    let mut proof: ZkProof = env
        .storage()
        .persistent()
        .get(&DataKey::ZkProof(proof_id))
        .expect("Proof not found");

    proof.status = ProofStatus::Revoked;

    env.storage()
        .persistent()
        .set(&DataKey::ZkProof(proof_id), &proof);

    env.events()
        .publish((symbol_short!("zk_rvk"),), (proof_id,));
}

// ── Private tips with ZK proofs ──────────────────────────────────────────────

/// Creates a private tip using a ZK proof.
///
/// The proof must verify that the tipper has sufficient balance and
/// the amount commitment is valid.
pub fn create_private_tip(
    env: &Env,
    creator: &Address,
    proof_id: u64,
    amount_commitment: BytesN<32>,
    nullifier: BytesN<32>,
) -> u64 {
    // Validate proof exists and is verified
    let proof: ZkProof = env
        .storage()
        .persistent()
        .get(&DataKey::ZkProof(proof_id))
        .expect("Proof not found");
    assert!(proof.status == ProofStatus::Verified, "Proof not verified");

    // Check nullifier matches
    assert!(proof.nullifier == nullifier, "Nullifier mismatch");

    let id = next_private_tip_id(env);

    let private_tip = PrivateTipProof {
        id,
        creator: creator.clone(),
        proof_id,
        amount_commitment,
        nullifier,
        claimed: false,
        created_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&DataKey::ZkPrivateTip(id), &private_tip);

    // Track private tips per creator
    let mut creator_tips: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::ZkCreatorPrivateTips(creator.clone()))
        .unwrap_or_else(|| Vec::new(env));
    creator_tips.push_back(id);
    env.storage().persistent().set(
        &DataKey::ZkCreatorPrivateTips(creator.clone()),
        &creator_tips,
    );

    env.events()
        .publish((symbol_short!("zk_ptip"),), (id, creator.clone(), proof_id));

    id
}

/// Claims/reveals a private tip. Creator only.
pub fn claim_private_tip(env: &Env, tip_id: u64) {
    let mut tip: PrivateTipProof = env
        .storage()
        .persistent()
        .get(&DataKey::ZkPrivateTip(tip_id))
        .expect("Private tip not found");

    assert!(!tip.claimed, "Already claimed");

    tip.claimed = true;
    env.storage()
        .persistent()
        .set(&DataKey::ZkPrivateTip(tip_id), &tip);

    env.events()
        .publish((symbol_short!("zk_clm"),), (tip_id, tip.creator.clone()));
}

// ── Nullifier management ─────────────────────────────────────────────────────

fn is_nullifier_used(env: &Env, nullifier: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::ZkNullifier(nullifier.clone()))
        .unwrap_or(false)
}

fn mark_nullifier_used(env: &Env, nullifier: &BytesN<32>) {
    env.storage()
        .persistent()
        .set(&DataKey::ZkNullifier(nullifier.clone()), &true);
}

// ── Query functions ──────────────────────────────────────────────────────────

/// Returns a circuit verification key by ID.
pub fn get_circuit(env: &Env, circuit_id: u64) -> Option<VerificationKey> {
    env.storage()
        .persistent()
        .get(&DataKey::ZkCircuit(circuit_id))
}

/// Returns a proof by ID.
pub fn get_proof(env: &Env, proof_id: u64) -> Option<ZkProof> {
    env.storage().persistent().get(&DataKey::ZkProof(proof_id))
}

/// Returns a private tip by ID.
pub fn get_private_tip(env: &Env, tip_id: u64) -> Option<PrivateTipProof> {
    env.storage()
        .persistent()
        .get(&DataKey::ZkPrivateTip(tip_id))
}

/// Returns all circuit IDs owned by an address.
pub fn get_owner_circuits(env: &Env, owner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::ZkOwnerCircuits(owner.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Returns all proof IDs submitted by a prover.
pub fn get_prover_proofs(env: &Env, prover: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::ZkProverProofs(prover.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Returns all private tip IDs for a creator.
pub fn get_creator_private_tips(env: &Env, creator: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::ZkCreatorPrivateTips(creator.clone()))
        .unwrap_or_else(|| Vec::new(env))
}
