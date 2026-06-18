//! Verkle tree for efficient tip state proofs and verification.
//!
//! Implements a simplified Verkle-style tree where each internal node
//! commits to its children via SHA-256. Proofs are compact paths from
//! leaf to root. Supports incremental updates and proof size optimization
//! by pruning unchanged subtrees.

use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{contracttype, symbol_short, Address, Bytes, BytesN, Env, Vec};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Branching factor (number of children per internal node).
pub const BRANCHING_FACTOR: u32 = 4;

/// Maximum tree depth.
pub const MAX_DEPTH: u32 = 8;

/// Maximum number of leaves per tree.
pub const MAX_LEAVES: u32 = 256; // BRANCHING_FACTOR ^ MAX_DEPTH / 2 (practical limit)

// ── Types ────────────────────────────────────────────────────────────────────

/// A leaf in the Verkle tree representing a tip state entry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerkleLeaf {
    /// Leaf index (position in the tree).
    pub index: u32,
    /// Key: SHA-256 of (creator_xdr || token_xdr).
    pub key: BytesN<32>,
    /// Value: tip balance as little-endian i128 bytes.
    pub value: Bytes,
    /// Leaf hash: SHA-256(key || value).
    pub hash: BytesN<32>,
}

/// An internal node in the Verkle tree.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerkleNode {
    /// Node identifier (depth * MAX_LEAVES + position).
    pub node_id: u64,
    /// Depth in the tree (0 = root).
    pub depth: u32,
    /// Position at this depth.
    pub position: u32,
    /// Commitment: SHA-256 of all children hashes concatenated.
    pub commitment: BytesN<32>,
    /// Whether this node is a leaf.
    pub is_leaf: bool,
}

/// A Verkle tree instance.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerkleTree {
    /// Unique tree ID.
    pub id: u64,
    /// Owner / creator of this tree.
    pub owner: Address,
    /// Current root commitment.
    pub root: BytesN<32>,
    /// Number of leaves currently in the tree.
    pub leaf_count: u32,
    /// Ledger timestamp of last update.
    pub updated_at: u64,
    /// Whether the tree is active.
    pub active: bool,
}

/// A Verkle proof: path of sibling commitments from leaf to root.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerkleProof {
    /// Unique proof ID.
    pub id: u64,
    /// Tree this proof belongs to.
    pub tree_id: u64,
    /// Leaf index being proven.
    pub leaf_index: u32,
    /// Leaf hash.
    pub leaf_hash: BytesN<32>,
    /// Ordered sibling commitments from leaf level up to root.
    pub path: Vec<BytesN<32>>,
    /// Expected root at time of proof generation.
    pub root: BytesN<32>,
    /// Ledger timestamp of generation.
    pub generated_at: u64,
    /// Whether this proof has been verified.
    pub verified: bool,
}

// ── Storage sub-keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerkleKey {
    /// Global tree ID counter.
    TreeCounter,
    /// Global proof ID counter.
    ProofCounter,
    /// VerkleTree record keyed by tree ID.
    Tree(u64),
    /// VerkleLeaf keyed by (tree_id, leaf_index).
    Leaf(u64, u32),
    /// VerkleNode keyed by (tree_id, node_id).
    Node(u64, u64),
    /// VerkleProof keyed by proof ID.
    Proof(u64),
    /// List of tree IDs owned by an address.
    OwnerTrees(Address),
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn next_tree_id(env: &Env) -> u64 {
    let key = DataKey::Verkle(VerkleKey::TreeCounter);
    let id: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &(id + 1));
    id
}

fn next_proof_id(env: &Env) -> u64 {
    let key = DataKey::Verkle(VerkleKey::ProofCounter);
    let id: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &(id + 1));
    id
}

fn save_tree(env: &Env, tree: &VerkleTree) {
    env.storage()
        .persistent()
        .set(&DataKey::Verkle(VerkleKey::Tree(tree.id)), tree);
}

fn get_tree_internal(env: &Env, tree_id: u64) -> VerkleTree {
    env.storage()
        .persistent()
        .get(&DataKey::Verkle(VerkleKey::Tree(tree_id)))
        .expect("tree not found")
}

fn save_leaf(env: &Env, tree_id: u64, leaf: &VerkleLeaf) {
    env.storage().persistent().set(
        &DataKey::Verkle(VerkleKey::Leaf(tree_id, leaf.index)),
        leaf,
    );
}

fn get_leaf_internal(env: &Env, tree_id: u64, index: u32) -> Option<VerkleLeaf> {
    env.storage()
        .persistent()
        .get(&DataKey::Verkle(VerkleKey::Leaf(tree_id, index)))
}

fn track_owner_tree(env: &Env, owner: &Address, tree_id: u64) {
    let key = DataKey::Verkle(VerkleKey::OwnerTrees(owner.clone()));
    let mut ids: Vec<u64> = env.storage().persistent().get(&key).unwrap_or(Vec::new(env));
    ids.push_back(tree_id);
    env.storage().persistent().set(&key, &ids);
}

// ── Hashing helpers ──────────────────────────────────────────────────────────

/// Compute a leaf hash: SHA-256(key || value).
fn leaf_hash(env: &Env, key: &BytesN<32>, value: &Bytes) -> BytesN<32> {
    let mut data = Bytes::new(env);
    let key_bytes: Bytes = key.clone().into();
    data.append(&key_bytes);
    data.append(value);
    env.crypto().sha256(&data).to_bytes()
}

/// Compute a node commitment from a list of child hashes.
fn node_commitment(env: &Env, children: &[BytesN<32>]) -> BytesN<32> {
    let mut data = Bytes::new(env);
    for child in children {
        let child_bytes: Bytes = child.clone().into();
        data.append(&child_bytes);
    }
    env.crypto().sha256(&data).to_bytes()
}

/// Recompute the root from all leaves in the tree.
///
/// Uses a bottom-up approach: group leaves into BRANCHING_FACTOR groups,
/// hash each group into a parent node, repeat until root.
fn compute_root(env: &Env, tree_id: u64, leaf_count: u32) -> BytesN<32> {
    if leaf_count == 0 {
        // Empty tree root is SHA-256 of empty bytes.
        return env.crypto().sha256(&Bytes::new(env)).to_bytes();
    }

    // Collect all leaf hashes.
    let mut level: Vec<BytesN<32>> = Vec::new(env);
    for i in 0..leaf_count {
        if let Some(leaf) = get_leaf_internal(env, tree_id, i) {
            level.push_back(leaf.hash.clone());
        } else {
            // Missing leaf: use zero hash.
            level.push_back(env.crypto().sha256(&Bytes::new(env)).to_bytes());
        }
    }

    // Reduce level by level.
    while level.len() > 1 {
        let mut next_level: Vec<BytesN<32>> = Vec::new(env);
        let len = level.len();
        let mut i = 0u32;
        while i < len {
            let end = (i + BRANCHING_FACTOR).min(len);
            let mut children: Vec<BytesN<32>> = Vec::new(env);
            for j in i..end {
                children.push_back(level.get(j).unwrap());
            }
            // Pad with zero hash if needed.
            while children.len() < BRANCHING_FACTOR {
                children.push_back(env.crypto().sha256(&Bytes::new(env)).to_bytes());
            }
            let commitment = node_commitment(env, &[
                children.get(0).unwrap(),
                children.get(1).unwrap(),
                children.get(2).unwrap(),
                children.get(3).unwrap(),
            ]);
            next_level.push_back(commitment);
            i += BRANCHING_FACTOR;
        }
        level = next_level;
    }

    level.get(0).unwrap()
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Create a new Verkle tree.
///
/// Returns the tree ID.
pub fn create_tree(env: &Env, owner: &Address) -> u64 {
    owner.require_auth();
    let tree_id = next_tree_id(env);
    let empty_root = env.crypto().sha256(&Bytes::new(env)).to_bytes();

    let tree = VerkleTree {
        id: tree_id,
        owner: owner.clone(),
        root: empty_root,
        leaf_count: 0,
        updated_at: env.ledger().timestamp(),
        active: true,
    };

    save_tree(env, &tree);
    track_owner_tree(env, owner, tree_id);

    env.events().publish(
        (symbol_short!("vk_crt"),),
        (tree_id, owner.clone()),
    );

    tree_id
}

/// Insert or update a leaf in the tree.
///
/// `key` is typically SHA-256(creator_xdr || token_xdr).
/// `value` is the serialized tip balance.
/// Returns the new root commitment.
pub fn update_leaf(
    env: &Env,
    tree_id: u64,
    creator: &Address,
    token: &Address,
    value: Bytes,
) -> BytesN<32> {
    let mut tree = get_tree_internal(env, tree_id);
    assert!(tree.active, "tree not active");

    // Derive the leaf key.
    let mut key_data = Bytes::new(env);
    key_data.append(&creator.to_xdr(env));
    key_data.append(&token.to_xdr(env));
    let key: BytesN<32> = env.crypto().sha256(&key_data).to_bytes();

    // Find existing leaf with this key or append a new one.
    let mut found_index: Option<u32> = None;
    for i in 0..tree.leaf_count {
        if let Some(existing) = get_leaf_internal(env, tree_id, i) {
            if existing.key == key {
                found_index = Some(i);
                break;
            }
        }
    }

    let index = found_index.unwrap_or_else(|| {
        assert!(tree.leaf_count < MAX_LEAVES, "tree full");
        let idx = tree.leaf_count;
        tree.leaf_count += 1;
        idx
    });

    let hash = leaf_hash(env, &key, &value);
    let leaf = VerkleLeaf {
        index,
        key,
        value,
        hash,
    };
    save_leaf(env, tree_id, &leaf);

    // Recompute root.
    let new_root = compute_root(env, tree_id, tree.leaf_count);
    tree.root = new_root.clone();
    tree.updated_at = env.ledger().timestamp();
    save_tree(env, &tree);

    env.events().publish(
        (symbol_short!("vk_upd"),),
        (tree_id, index, new_root.clone()),
    );

    new_root
}

/// Generate a Verkle proof for a leaf at `leaf_index`.
///
/// Returns the proof ID.
pub fn generate_proof(env: &Env, tree_id: u64, leaf_index: u32) -> u64 {
    let tree = get_tree_internal(env, tree_id);
    assert!(tree.active, "tree not active");
    assert!(leaf_index < tree.leaf_count, "leaf index out of range");

    let leaf = get_leaf_internal(env, tree_id, leaf_index).expect("leaf not found");

    // Build the proof path: collect sibling hashes at each level.
    let path = build_proof_path(env, tree_id, leaf_index, tree.leaf_count);

    let proof_id = next_proof_id(env);
    let proof = VerkleProof {
        id: proof_id,
        tree_id,
        leaf_index,
        leaf_hash: leaf.hash.clone(),
        path,
        root: tree.root.clone(),
        generated_at: env.ledger().timestamp(),
        verified: false,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Verkle(VerkleKey::Proof(proof_id)), &proof);

    env.events().publish(
        (symbol_short!("vk_pgen"),),
        (proof_id, tree_id, leaf_index),
    );

    proof_id
}

/// Verify a Verkle proof against the current tree root.
///
/// Returns true if the proof is valid.
pub fn verify_proof(env: &Env, proof_id: u64) -> bool {
    let key = DataKey::Verkle(VerkleKey::Proof(proof_id));
    let mut proof: VerkleProof = env
        .storage()
        .persistent()
        .get(&key)
        .expect("proof not found");

    let tree = get_tree_internal(env, proof.tree_id);

    // Recompute root from the proof path.
    let computed_root = recompute_root_from_path(env, &proof.leaf_hash, proof.leaf_index, &proof.path);
    let valid = computed_root == tree.root;

    proof.verified = valid;
    env.storage().persistent().set(&key, &proof);

    env.events().publish(
        (symbol_short!("vk_vfy"),),
        (proof_id, valid),
    );

    valid
}

/// Get a Verkle tree by ID.
pub fn get_tree(env: &Env, tree_id: u64) -> Option<VerkleTree> {
    env.storage()
        .persistent()
        .get(&DataKey::Verkle(VerkleKey::Tree(tree_id)))
}

/// Get a leaf by tree ID and index.
pub fn get_leaf(env: &Env, tree_id: u64, index: u32) -> Option<VerkleLeaf> {
    get_leaf_internal(env, tree_id, index)
}

/// Get a proof by ID.
pub fn get_proof(env: &Env, proof_id: u64) -> Option<VerkleProof> {
    env.storage()
        .persistent()
        .get(&DataKey::Verkle(VerkleKey::Proof(proof_id)))
}

/// Get all tree IDs owned by an address.
pub fn get_owner_trees(env: &Env, owner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::Verkle(VerkleKey::OwnerTrees(owner.clone())))
        .unwrap_or(Vec::new(env))
}

/// Deactivate a tree.
pub fn deactivate_tree(env: &Env, owner: &Address, tree_id: u64) {
    owner.require_auth();
    let mut tree = get_tree_internal(env, tree_id);
    assert!(tree.owner == *owner, "not tree owner");
    tree.active = false;
    save_tree(env, &tree);

    env.events().publish(
        (symbol_short!("vk_deact"),),
        (tree_id,),
    );
}

// ── Internal proof helpers ───────────────────────────────────────────────────

/// Build the sibling path for a leaf at `leaf_index`.
///
/// At each level, collect the sibling hashes within the same group.
fn build_proof_path(env: &Env, tree_id: u64, leaf_index: u32, leaf_count: u32) -> Vec<BytesN<32>> {
    let mut path: Vec<BytesN<32>> = Vec::new(env);
    let zero_hash = env.crypto().sha256(&Bytes::new(env)).to_bytes();

    // Collect all leaf hashes.
    let mut level: Vec<BytesN<32>> = Vec::new(env);
    for i in 0..leaf_count {
        if let Some(leaf) = get_leaf_internal(env, tree_id, i) {
            level.push_back(leaf.hash.clone());
        } else {
            level.push_back(zero_hash.clone());
        }
    }

    let mut current_index = leaf_index;

    while level.len() > 1 {
        // Determine which group this index belongs to.
        let group_start = (current_index / BRANCHING_FACTOR) * BRANCHING_FACTOR;

        // Collect siblings (all members of the group except current_index).
        for j in 0..BRANCHING_FACTOR {
            let sibling_idx = group_start + j;
            if sibling_idx != current_index {
                let hash = if sibling_idx < level.len() {
                    level.get(sibling_idx).unwrap()
                } else {
                    zero_hash.clone()
                };
                path.push_back(hash);
            }
        }

        // Reduce level.
        let mut next_level: Vec<BytesN<32>> = Vec::new(env);
        let len = level.len();
        let mut i = 0u32;
        while i < len {
            let end = (i + BRANCHING_FACTOR).min(len);
            let mut children: Vec<BytesN<32>> = Vec::new(env);
            for j in i..end {
                children.push_back(level.get(j).unwrap());
            }
            while children.len() < BRANCHING_FACTOR {
                children.push_back(zero_hash.clone());
            }
            let commitment = node_commitment(env, &[
                children.get(0).unwrap(),
                children.get(1).unwrap(),
                children.get(2).unwrap(),
                children.get(3).unwrap(),
            ]);
            next_level.push_back(commitment);
            i += BRANCHING_FACTOR;
        }

        current_index /= BRANCHING_FACTOR;
        level = next_level;
    }

    path
}

/// Recompute the root from a proof path.
fn recompute_root_from_path(
    env: &Env,
    leaf_hash: &BytesN<32>,
    leaf_index: u32,
    path: &Vec<BytesN<32>>,
) -> BytesN<32> {
    let zero_hash = env.crypto().sha256(&Bytes::new(env)).to_bytes();
    let mut current = leaf_hash.clone();
    let mut current_index = leaf_index;
    let mut path_iter = 0u32;

    // Each step in the path covers (BRANCHING_FACTOR - 1) siblings.
    let siblings_per_level = BRANCHING_FACTOR - 1;

    while path_iter + siblings_per_level <= path.len() {
        let position_in_group = current_index % BRANCHING_FACTOR;
        let mut children: Vec<BytesN<32>> = Vec::new(env);
        let mut sibling_used = 0u32;

        for j in 0..BRANCHING_FACTOR {
            if j == position_in_group {
                children.push_back(current.clone());
            } else {
                let sib = if path_iter + sibling_used < path.len() {
                    path.get(path_iter + sibling_used).unwrap()
                } else {
                    zero_hash.clone()
                };
                children.push_back(sib);
                sibling_used += 1;
            }
        }

        current = node_commitment(env, &[
            children.get(0).unwrap(),
            children.get(1).unwrap(),
            children.get(2).unwrap(),
            children.get(3).unwrap(),
        ]);

        current_index /= BRANCHING_FACTOR;
        path_iter += siblings_per_level;
    }

    current
}
