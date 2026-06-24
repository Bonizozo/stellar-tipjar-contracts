/// Recursive proof composition for efficient verification of tip chains.
use soroban_sdk::{contracttype, Address, BytesN, Env, Vec};

/// A proof node in the recursive composition tree.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofNode {
    pub tip_id: u64,
    pub creator: Address,
    pub amount: i128,
    pub hash: BytesN<32>,
}

/// Recursive proof for a chain of tips.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecursiveProof {
    pub proof_id: u64,
    pub root_hash: BytesN<32>,
    pub tip_count: u32,
    pub total_amount: i128,
    pub created_at: u64,
}

/// Generates a proof for a single tip.
pub fn generate_tip_proof(env: &Env, tip_id: u64, creator: &Address, amount: i128) -> ProofNode {
    let mut data = soroban_sdk::Bytes::new(env);
    data.append(&tip_id.to_be_bytes().into());
    data.append(&creator.to_string().as_bytes());
    data.append(&amount.to_be_bytes().into());
    
    let hash = env.crypto().sha256(&data);
    
    ProofNode {
        tip_id,
        creator: creator.clone(),
        amount,
        hash,
    }
}

/// Composes two proof nodes into a parent node using merkle-style hashing.
pub fn compose_proofs(env: &Env, left: &ProofNode, right: &ProofNode) -> BytesN<32> {
    let mut combined = soroban_sdk::Bytes::new(env);
    combined.append(&left.hash.to_bytes());
    combined.append(&right.hash.to_bytes());
    env.crypto().sha256(&combined)
}

/// Verifies a proof chain by recomputing the root hash.
pub fn verify_proof_chain(env: &Env, nodes: &Vec<ProofNode>, expected_root: &BytesN<32>) -> bool {
    if nodes.is_empty() {
        return false;
    }
    
    if nodes.len() == 1 {
        return &nodes.get(0).unwrap().hash == expected_root;
    }
    
    let mut current_level: Vec<BytesN<32>> = Vec::new(env);
    for node in nodes.iter() {
        current_level.push_back(node.hash.clone());
    }
    
    while current_level.len() > 1 {
        let mut next_level: Vec<BytesN<32>> = Vec::new(env);
        let mut i = 0;
        
        while i < current_level.len() {
            if i + 1 < current_level.len() {
                let left = current_level.get(i).unwrap();
                let right = current_level.get(i + 1).unwrap();
                let mut combined = soroban_sdk::Bytes::new(env);
                combined.append(&left.to_bytes());
                combined.append(&right.to_bytes());
                next_level.push_back(env.crypto().sha256(&combined));
                i += 2;
            } else {
                next_level.push_back(current_level.get(i).unwrap());
                i += 1;
            }
        }
        
        current_level = next_level;
    }
    
    &current_level.get(0).unwrap() == expected_root
}

/// Optimizes proof size by aggregating consecutive tips.
pub fn optimize_proof(env: &Env, nodes: &Vec<ProofNode>) -> Vec<ProofNode> {
    if nodes.len() <= 2 {
        return nodes.clone();
    }
    
    let mut optimized: Vec<ProofNode> = Vec::new(env);
    let mut i = 0;
    
    while i < nodes.len() {
        if i + 1 < nodes.len() {
            let left = nodes.get(i).unwrap();
            let right = nodes.get(i + 1).unwrap();
            
            let combined_hash = compose_proofs(env, &left, &right);
            let combined_node = ProofNode {
                tip_id: left.tip_id,
                creator: left.creator.clone(),
                amount: left.amount + right.amount,
                hash: combined_hash,
            };
            
            optimized.push_back(combined_node);
            i += 2;
        } else {
            optimized.push_back(nodes.get(i).unwrap());
            i += 1;
        }
    }
    
    optimized
}
