//! Tip validity proof system.
//!
//! Allows verifying tip transactions without full re-execution by generating
//! and verifying compact proofs. Supports batching and aggregation.

use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{contracttype, symbol_short, Address, Bytes, BytesN, Env, Vec};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Maximum number of proofs in a single batch/aggregate.
pub const MAX_PROOF_BATCH: u32 = 64;

// ── Types ────────────────────────────────────────────────────────────────────

/// Validity status of a proof.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofValidity {
    /// Proof has been generated but not yet verified.
    Pending,
    /// Proof has been verified as valid.
    Valid,
    /// Proof failed verification.
    Invalid,
    /// Proof has been revoked.
    Revoked,
}

/// A validity proof for a single tip transaction.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipProof {
    /// Unique proof ID.
    pub id: u64,
    /// The tip transaction ID this proof covers.
    pub tip_id: u64,
    /// Sender address.
    pub sender: Address,
    /// Creator / recipient address.
    pub creator: Address,
    /// Token address.
    pub token: Address,
    /// Tip amount.
    pub amount: i128,
    /// SHA-256 commitment: hash(sender_xdr || creator_xdr || token_xdr || amount_le || nonce).
    pub commitment: BytesN<32>,
    /// Proof witness bytes (e.g. signature or Merkle path).
    pub witness: Bytes,
    /// Current validity status.
    pub validity: ProofValidity,
    /// Ledger timestamp of generation.
    pub generated_at: u64,
    /// Ledger timestamp of verification (0 if not yet verified).
    pub verified_at: u64,
    /// Nonce used in commitment to prevent replay.
    pub nonce: u64,
}

/// An aggregated proof covering multiple tip proofs.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AggregatedProof {
    /// Unique aggregate ID.
    pub id: u64,
    /// Ordered list of proof IDs included.
    pub proof_ids: Vec<u64>,
    /// Aggregate root: SHA-256 of all individual commitments concatenated.
    pub aggregate_root: BytesN<32>,
    /// Overall validity.
    pub validity: ProofValidity,
    /// Ledger timestamp of aggregation.
    pub created_at: u64,
    /// Number of valid proofs within the aggregate.
    pub valid_count: u32,
}

// ── Storage sub-keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidityProofKey {
    /// Global proof ID counter.
    Counter,
    /// Global aggregate ID counter.
    AggCounter,
    /// TipProof keyed by proof ID.
    Proof(u64),
    /// AggregatedProof keyed by aggregate ID.
    Aggregate(u64),
    /// Proof ID for a given tip_id (one proof per tip).
    TipProofId(u64),
    /// Nonce for a given sender (replay protection).
    Nonce(Address),
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn next_proof_id(env: &Env) -> u64 {
    let key = DataKey::ValidityProof(ValidityProofKey::Counter);
    let id: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &(id + 1));
    id
}

fn next_agg_id(env: &Env) -> u64 {
    let key = DataKey::ValidityProof(ValidityProofKey::AggCounter);
    let id: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &(id + 1));
    id
}

fn get_nonce(env: &Env, sender: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::ValidityProof(ValidityProofKey::Nonce(
            sender.clone(),
        )))
        .unwrap_or(0)
}

fn bump_nonce(env: &Env, sender: &Address) -> u64 {
    let nonce = get_nonce(env, sender);
    env.storage().persistent().set(
        &DataKey::ValidityProof(ValidityProofKey::Nonce(sender.clone())),
        &(nonce + 1),
    );
    nonce
}

// ── Commitment construction ──────────────────────────────────────────────────

/// Build the commitment hash for a tip proof.
///
/// commitment = SHA-256(sender_xdr || creator_xdr || token_xdr || amount_le8 || nonce_le8)
fn build_commitment(
    env: &Env,
    sender: &Address,
    creator: &Address,
    token: &Address,
    amount: i128,
    nonce: u64,
) -> BytesN<32> {
    use soroban_sdk::Bytes;
    let mut data = Bytes::new(env);
    data.append(&sender.to_xdr(env));
    data.append(&creator.to_xdr(env));
    data.append(&token.to_xdr(env));
    data.append(&Bytes::from_array(env, &amount.to_le_bytes()));
    data.append(&Bytes::from_array(env, &nonce.to_le_bytes()));
    env.crypto().sha256(&data).to_bytes()
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Generate a validity proof for a tip transaction.
///
/// Returns the proof ID.
pub fn generate_proof(
    env: &Env,
    sender: &Address,
    creator: &Address,
    token: &Address,
    amount: i128,
    tip_id: u64,
    witness: Bytes,
) -> u64 {
    sender.require_auth();
    assert!(amount > 0, "amount must be positive");

    let nonce = bump_nonce(env, sender);
    let commitment = build_commitment(env, sender, creator, token, amount, nonce);
    let proof_id = next_proof_id(env);
    let now = env.ledger().timestamp();

    let proof = TipProof {
        id: proof_id,
        tip_id,
        sender: sender.clone(),
        creator: creator.clone(),
        token: token.clone(),
        amount,
        commitment: commitment.clone(),
        witness,
        validity: ProofValidity::Pending,
        generated_at: now,
        verified_at: 0,
        nonce,
    };

    env.storage()
        .persistent()
        .set(&DataKey::ValidityProof(ValidityProofKey::Proof(proof_id)), &proof);
    env.storage().persistent().set(
        &DataKey::ValidityProof(ValidityProofKey::TipProofId(tip_id)),
        &proof_id,
    );

    env.events().publish(
        (symbol_short!("vp_gen"),),
        (proof_id, tip_id, commitment.clone()),
    );

    proof_id
}

/// Verify a validity proof.
///
/// Re-derives the commitment from stored proof data and compares.
/// Updates the proof status to `Valid` or `Invalid`.
pub fn verify_proof(env: &Env, proof_id: u64) -> bool {
    let key = DataKey::ValidityProof(ValidityProofKey::Proof(proof_id));
    let mut proof: TipProof = env
        .storage()
        .persistent()
        .get(&key)
        .expect("proof not found");

    assert!(
        matches!(proof.validity, ProofValidity::Pending),
        "proof already verified"
    );

    let expected = build_commitment(
        env,
        &proof.sender,
        &proof.creator,
        &proof.token,
        proof.amount,
        proof.nonce,
    );

    let valid = expected == proof.commitment;
    proof.validity = if valid {
        ProofValidity::Valid
    } else {
        ProofValidity::Invalid
    };
    proof.verified_at = env.ledger().timestamp();
    env.storage().persistent().set(&key, &proof);

    env.events().publish(
        (symbol_short!("vp_vrfy"),),
        (proof_id, valid),
    );

    valid
}

/// Batch-verify multiple proofs.
///
/// Returns the number of valid proofs.
pub fn batch_verify(env: &Env, proof_ids: Vec<u64>) -> u32 {
    assert!(proof_ids.len() <= MAX_PROOF_BATCH, "batch too large");
    let mut valid_count: u32 = 0;
    for id in proof_ids.iter() {
        if verify_proof(env, id) {
            valid_count += 1;
        }
    }
    valid_count
}

/// Aggregate multiple proofs into a single aggregate proof.
///
/// Returns the aggregate ID.
pub fn aggregate_proofs(env: &Env, proof_ids: Vec<u64>) -> u64 {
    assert!(!proof_ids.is_empty(), "no proofs to aggregate");
    assert!(proof_ids.len() <= MAX_PROOF_BATCH, "batch too large");

    let mut combined = soroban_sdk::Bytes::new(env);
    let mut valid_count: u32 = 0;

    for id in proof_ids.iter() {
        let proof: TipProof = env
            .storage()
            .persistent()
            .get(&DataKey::ValidityProof(ValidityProofKey::Proof(id)))
            .expect("proof not found");

        if matches!(proof.validity, ProofValidity::Valid) {
            valid_count += 1;
        }
        // Append commitment bytes to combined payload.
        let commitment_bytes: soroban_sdk::Bytes = proof.commitment.into();
        combined.append(&commitment_bytes);
    }

    let aggregate_root: BytesN<32> = env.crypto().sha256(&combined).to_bytes();
    let agg_id = next_agg_id(env);
    let now = env.ledger().timestamp();

    let agg = AggregatedProof {
        id: agg_id,
        proof_ids: proof_ids.clone(),
        aggregate_root: aggregate_root.clone(),
        validity: if valid_count == proof_ids.len() as u32 {
            ProofValidity::Valid
        } else {
            ProofValidity::Invalid
        },
        created_at: now,
        valid_count,
    };

    env.storage().persistent().set(
        &DataKey::ValidityProof(ValidityProofKey::Aggregate(agg_id)),
        &agg,
    );

    env.events().publish(
        (symbol_short!("vp_agg"),),
        (agg_id, proof_ids.len(), valid_count, aggregate_root.clone()),
    );

    agg_id
}

/// Revoke a proof (e.g. if the underlying tip was reversed).
pub fn revoke_proof(env: &Env, admin: &Address, proof_id: u64) {
    admin.require_auth();
    let key = DataKey::ValidityProof(ValidityProofKey::Proof(proof_id));
    let mut proof: TipProof = env
        .storage()
        .persistent()
        .get(&key)
        .expect("proof not found");
    proof.validity = ProofValidity::Revoked;
    env.storage().persistent().set(&key, &proof);

    env.events().publish(
        (symbol_short!("vp_rev"),),
        (proof_id,),
    );
}

/// Retrieve a proof by ID.
pub fn get_proof(env: &Env, proof_id: u64) -> Option<TipProof> {
    env.storage()
        .persistent()
        .get(&DataKey::ValidityProof(ValidityProofKey::Proof(proof_id)))
}

/// Retrieve the proof ID for a given tip ID.
pub fn get_proof_for_tip(env: &Env, tip_id: u64) -> Option<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::ValidityProof(ValidityProofKey::TipProofId(tip_id)))
}

/// Retrieve an aggregated proof by ID.
pub fn get_aggregate(env: &Env, agg_id: u64) -> Option<AggregatedProof> {
    env.storage()
        .persistent()
        .get(&DataKey::ValidityProof(ValidityProofKey::Aggregate(agg_id)))
}
