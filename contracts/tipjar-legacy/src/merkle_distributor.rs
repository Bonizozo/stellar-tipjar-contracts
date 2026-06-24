//! Merkle distributor module (#248).
//!
//! Gas-efficient batch tip claims using a Merkle tree.
//! The admin commits a Merkle root; claimants supply a proof to claim their allocation.

use soroban_sdk::{contracttype, symbol_short, token, Address, Bytes, BytesN, Env, Vec};

use crate::DataKey;

// ── Types ────────────────────────────────────────────────────────────────────

/// A Merkle distribution campaign.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MerkleDistribution {
    pub id: u64,
    /// SHA-256 Merkle root of (recipient || amount) leaves.
    pub root: BytesN<32>,
    pub token: Address,
    /// Total tokens deposited for this distribution.
    pub total_amount: i128,
    /// Tokens already claimed.
    pub claimed_amount: i128,
    pub created_at: u64,
    pub active: bool,
}

// ── DataKey sub-keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MerkleKey {
    Counter,
    Distribution(u64),
    /// Whether (distribution_id, recipient) has already claimed.
    Claimed(u64, Address),
}

// ── Leaf hashing ─────────────────────────────────────────────────────────────

/// Compute the leaf hash for `(recipient, amount)`.
///
/// Leaf = SHA-256(recipient_xdr_bytes || amount_le_bytes)
fn leaf_hash(env: &Env, recipient: &Address, amount: i128) -> BytesN<32> {
    let mut data = Bytes::new(env);
    // Encode recipient via XDR serialisation.
    let addr_bytes = recipient.to_xdr(env);
    data.append(&addr_bytes);
    let amount_bytes = Bytes::from_array(env, &amount.to_le_bytes());
    data.append(&amount_bytes);
    env.crypto().sha256(&data)
}

/// Verify a Merkle proof.
///
/// `proof` is an ordered list of sibling hashes from leaf to root.
/// Standard binary Merkle tree: at each level, sort the two children
/// (smaller hash first) before hashing.
pub fn verify_proof(
    env: &Env,
    root: &BytesN<32>,
    recipient: &Address,
    amount: i128,
    proof: &Vec<BytesN<32>>,
) -> bool {
    let mut current = leaf_hash(env, recipient, amount);

    for sibling in proof.iter() {
        // Sort: smaller hash goes first to ensure deterministic ordering.
        // Compare by converting to Bytes for ordering.
        let mut data = Bytes::new(env);
        let cur_bytes: Bytes = current.clone().into();
        let sib_bytes: Bytes = sibling.clone().into();
        if cur_bytes <= sib_bytes {
            data.append(&cur_bytes);
            data.append(&sib_bytes);
        } else {
            data.append(&sib_bytes);
            data.append(&cur_bytes);
        }
        current = env.crypto().sha256(&data);
    }

    &current == root
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Create a new Merkle distribution.
///
/// Transfers `total_amount` of `token` from `creator` into escrow.
/// Returns the distribution ID.
/// Emits `("md_create",)` with `(id, root, token, total_amount)`.
pub fn create_distribution(
    env: &Env,
    creator: &Address,
    token_addr: &Address,
    root: BytesN<32>,
    total_amount: i128,
) -> u64 {
    assert!(total_amount > 0, "amount must be positive");

    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::MerkleDistributor(MerkleKey::Counter))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DataKey::MerkleDistributor(MerkleKey::Counter), &(id + 1));

    let dist = MerkleDistribution {
        id,
        root,
        token: token_addr.clone(),
        total_amount,
        claimed_amount: 0,
        created_at: env.ledger().timestamp(),
        active: true,
    };

    env.storage()
        .persistent()
        .set(&DataKey::MerkleDistributor(MerkleKey::Distribution(id)), &dist);

    token::Client::new(env, token_addr).transfer(creator, &env.current_contract_address(), &total_amount);

    env.events().publish(
        (symbol_short!("md_crt"),),
        (id, dist.root.clone(), token_addr.clone(), total_amount),
    );

    id
}

/// Claim an allocation from a Merkle distribution.
///
/// Verifies the proof, marks the claim, and transfers `amount` to `recipient`.
/// Panics if already claimed, proof invalid, or insufficient funds.
/// Emits `("md_claim",)` with `(distribution_id, recipient, amount)`.
pub fn claim(
    env: &Env,
    distribution_id: u64,
    recipient: &Address,
    amount: i128,
    proof: Vec<BytesN<32>>,
) {
    assert!(amount > 0, "amount must be positive");

    let claimed_key = DataKey::MerkleDistributor(MerkleKey::Claimed(distribution_id, recipient.clone()));
    let already_claimed: bool = env.storage().persistent().get(&claimed_key).unwrap_or(false);
    assert!(!already_claimed, "already claimed");

    let mut dist: MerkleDistribution = env
        .storage()
        .persistent()
        .get(&DataKey::MerkleDistributor(MerkleKey::Distribution(distribution_id)))
        .unwrap_or_else(|| panic!("distribution not found"));

    assert!(dist.active, "distribution not active");
    assert!(
        verify_proof(env, &dist.root, recipient, amount, &proof),
        "invalid proof"
    );
    assert!(
        dist.claimed_amount.saturating_add(amount) <= dist.total_amount,
        "insufficient funds"
    );

    // Mark claimed before transfer (CEI).
    env.storage().persistent().set(&claimed_key, &true);
    dist.claimed_amount = dist.claimed_amount.saturating_add(amount);
    env.storage()
        .persistent()
        .set(&DataKey::MerkleDistributor(MerkleKey::Distribution(distribution_id)), &dist);

    token::Client::new(env, &dist.token).transfer(
        &env.current_contract_address(),
        recipient,
        &amount,
    );

    env.events().publish(
        (symbol_short!("md_clm"),),
        (distribution_id, recipient.clone(), amount),
    );
}

/// Deactivate a distribution (admin). Remaining unclaimed tokens stay in contract.
/// Emits `("md_deact",)` with `(distribution_id,)`.
pub fn deactivate(env: &Env, distribution_id: u64) {
    let mut dist: MerkleDistribution = env
        .storage()
        .persistent()
        .get(&DataKey::MerkleDistributor(MerkleKey::Distribution(distribution_id)))
        .unwrap_or_else(|| panic!("distribution not found"));
    dist.active = false;
    env.storage()
        .persistent()
        .set(&DataKey::MerkleDistributor(MerkleKey::Distribution(distribution_id)), &dist);
    env.events().publish((symbol_short!("md_dea"),), (distribution_id,));
}

/// Returns whether `recipient` has already claimed from `distribution_id`.
pub fn is_claimed(env: &Env, distribution_id: u64, recipient: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::MerkleDistributor(MerkleKey::Claimed(distribution_id, recipient.clone())))
        .unwrap_or(false)
}

/// Returns the distribution record.
pub fn get_distribution(env: &Env, distribution_id: u64) -> MerkleDistribution {
    env.storage()
        .persistent()
        .get(&DataKey::MerkleDistributor(MerkleKey::Distribution(distribution_id)))
        .unwrap_or_else(|| panic!("distribution not found"))
}
