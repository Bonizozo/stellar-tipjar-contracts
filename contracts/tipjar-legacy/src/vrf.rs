/// Verifiable Random Function for provably fair random selection.
use soroban_sdk::{contracttype, Address, BytesN, Env, Vec};

/// VRF proof for verifiable randomness.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VrfProof {
    pub seed: BytesN<32>,
    pub proof: BytesN<32>,
    pub output: BytesN<32>,
    pub timestamp: u64,
}

/// VRF lottery configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VrfLottery {
    pub lottery_id: u64,
    pub creator: Address,
    pub participants: Vec<Address>,
    pub prize_amount: i128,
    pub vrf_seed: BytesN<32>,
    pub winner: Option<Address>,
    pub proof: Option<VrfProof>,
    pub finalized: bool,
}

/// Generates VRF proof from seed and secret.
pub fn generate_proof(env: &Env, seed: &BytesN<32>, secret: &BytesN<32>) -> VrfProof {
    let mut combined = soroban_sdk::Bytes::new(env);
    combined.append(&seed.to_bytes());
    combined.append(&secret.to_bytes());
    
    let proof = env.crypto().sha256(&combined);
    let output = env.crypto().sha256(&proof.to_bytes());
    
    VrfProof {
        seed: seed.clone(),
        proof,
        output,
        timestamp: env.ledger().timestamp(),
    }
}

/// Verifies VRF proof integrity.
pub fn verify_proof(env: &Env, proof: &VrfProof, secret: &BytesN<32>) -> bool {
    let mut combined = soroban_sdk::Bytes::new(env);
    combined.append(&proof.seed.to_bytes());
    combined.append(&secret.to_bytes());
    
    let expected_proof = env.crypto().sha256(&combined);
    let expected_output = env.crypto().sha256(&expected_proof.to_bytes());
    
    proof.proof == expected_proof && proof.output == expected_output
}

/// Generates random number from VRF output.
pub fn generate_random(output: &BytesN<32>, max: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    
    let bytes = output.to_array();
    let mut value: u32 = 0;
    
    for i in 0..4 {
        value = (value << 8) | (bytes[i] as u32);
    }
    
    value % max
}

/// Selects winner from participants using VRF.
pub fn select_winner(env: &Env, participants: &Vec<Address>, vrf_output: &BytesN<32>) -> Address {
    let count = participants.len();
    let index = generate_random(vrf_output, count);
    participants.get(index).unwrap()
}

/// Verifies fairness by checking proof and selection.
pub fn verify_fairness(
    env: &Env,
    proof: &VrfProof,
    secret: &BytesN<32>,
    participants: &Vec<Address>,
    winner: &Address,
) -> bool {
    if !verify_proof(env, proof, secret) {
        return false;
    }
    
    let expected_winner = select_winner(env, participants, &proof.output);
    &expected_winner == winner
}
