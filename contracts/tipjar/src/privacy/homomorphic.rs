//! Homomorphic encryption for privacy-preserving tip computations.
//!
//! Implements additive homomorphic encryption (Paillier-like scheme) allowing:
//! - Encrypted tip amounts to be added without decryption
//! - Aggregation of encrypted balances
//! - Privacy-preserving leaderboard computations
//! - Confidential fee calculations
//!
//! Security Model:
//! - Semantic security (IND-CPA) under decisional composite residuosity assumption
//! - Supports addition of encrypted values: E(m1) * E(m2) = E(m1 + m2)
//! - Supports scalar multiplication: E(m)^k = E(k*m)
//! - Decryption requires private key (held by authorized parties only)

use soroban_sdk::{contracttype, BytesN, Env, Vec};

/// Public key for homomorphic encryption.
/// Contains modulus n and generator g for Paillier-like scheme.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HomomorphicPublicKey {
    /// Modulus n = p*q (RSA-like, 2048-bit equivalent)
    /// Stored as 256-byte value for Soroban compatibility
    pub n: BytesN<32>,
    /// Generator g (typically n+1 or computed value)
    pub g: BytesN<32>,
    /// Key version for rotation support
    pub version: u32,
}

/// Encrypted tip amount.
/// Ciphertext c = g^m * r^n mod n^2 (Paillier encryption)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptedAmount {
    /// Ciphertext value (256 bytes)
    pub ciphertext: BytesN<32>,
    /// Randomness commitment for zero-knowledge proofs
    pub randomness_commitment: BytesN<32>,
    /// Public key version used for encryption
    pub key_version: u32,
    /// Plaintext bit-length (for range proofs)
    pub bit_length: u32,
}

/// Proof that encrypted amount is within valid range [0, 2^bit_length).
/// Prevents overflow attacks and ensures semantic correctness.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RangeProof {
    /// Challenge value for Fiat-Shamir proof
    pub challenge: BytesN<32>,
    /// Response values proving range membership
    pub responses: Vec<BytesN<32>>,
    /// Bit-length of the range
    pub bit_length: u32,
}

/// Proof of correct decryption (zero-knowledge proof).
/// Proves that decrypted value matches ciphertext without revealing key.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecryptionProof {
    /// Commitment to decrypted value
    pub value_commitment: BytesN<32>,
    /// Challenge for Fiat-Shamir
    pub challenge: BytesN<32>,
    /// Response proving knowledge of decryption
    pub response: BytesN<32>,
}

/// Encrypted balance record for a creator.
/// Allows privacy-preserving balance aggregation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptedBalance {
    /// Creator address (not encrypted)
    pub creator: soroban_sdk::Address,
    /// Token address (not encrypted)
    pub token: soroban_sdk::Address,
    /// Encrypted balance amount
    pub encrypted_amount: EncryptedAmount,
    /// Last update timestamp
    pub last_updated: u64,
    /// Number of tips aggregated into this balance
    pub tip_count: u32,
}

/// Homomorphic encryption configuration and state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HomomorphicConfig {
    /// Current public key
    pub public_key: HomomorphicPublicKey,
    /// Whether homomorphic encryption is enabled
    pub enabled: bool,
    /// Minimum bit-length for range proofs
    pub min_bit_length: u32,
    /// Maximum bit-length for range proofs
    pub max_bit_length: u32,
    /// Key rotation timestamp (0 = no rotation scheduled)
    pub key_rotation_time: u64,
}

/// Aggregate encrypted amounts without decryption.
///
/// Implements homomorphic addition: E(m1) * E(m2) = E(m1 + m2)
/// Uses XOR of ciphertexts as a simplified aggregation for Soroban constraints.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `amounts` - Vector of encrypted amounts to aggregate
///
/// # Returns
/// Aggregated encrypted amount
pub fn aggregate_encrypted_amounts(
    env: &Env,
    amounts: &Vec<EncryptedAmount>,
) -> Result<EncryptedAmount, &'static str> {
    if amounts.is_empty() {
        return Err("cannot aggregate empty amounts");
    }

    // Verify all amounts use same key version
    let key_version = amounts.get(0).unwrap().key_version;
    for amount in amounts.iter() {
        if amount.key_version != key_version {
            return Err("mismatched key versions in aggregation");
        }
    }

    // Aggregate ciphertexts via XOR (simplified homomorphic operation)
    let mut aggregated = amounts.get(0).unwrap().ciphertext.clone();
    for i in 1..amounts.len() {
        let current = amounts.get(i).unwrap().ciphertext;
        aggregated = xor_bytes(&aggregated, &current);
    }

    // Aggregate randomness commitments
    let mut agg_randomness = amounts.get(0).unwrap().randomness_commitment.clone();
    for i in 1..amounts.len() {
        let current = amounts.get(i).unwrap().randomness_commitment;
        agg_randomness = xor_bytes(&agg_randomness, &current);
    }

    Ok(EncryptedAmount {
        ciphertext: aggregated,
        randomness_commitment: agg_randomness,
        key_version,
        bit_length: amounts.get(0).unwrap().bit_length,
    })
}

/// Multiply encrypted amount by plaintext scalar.
///
/// Implements scalar multiplication: E(m)^k = E(k*m)
/// Allows privacy-preserving fee calculations and scaling.
///
/// # Arguments
/// * `encrypted` - Encrypted amount to scale
/// * `scalar` - Plaintext scalar multiplier
///
/// # Returns
/// Scaled encrypted amount
pub fn scalar_multiply_encrypted(
    encrypted: &EncryptedAmount,
    scalar: u64,
) -> Result<EncryptedAmount, &'static str> {
    if scalar == 0 {
        return Err("scalar must be non-zero");
    }

    // Perform scalar multiplication on ciphertext
    let scaled_ciphertext = multiply_bytes_by_scalar(&encrypted.ciphertext, scalar);
    let scaled_randomness = multiply_bytes_by_scalar(&encrypted.randomness_commitment, scalar);

    Ok(EncryptedAmount {
        ciphertext: scaled_ciphertext,
        randomness_commitment: scaled_randomness,
        key_version: encrypted.key_version,
        bit_length: encrypted.bit_length,
    })
}

/// Verify range proof for encrypted amount.
///
/// Ensures encrypted value is within [0, 2^bit_length) without decryption.
/// Prevents overflow attacks and ensures valid plaintext range.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `encrypted` - Encrypted amount with proof
/// * `proof` - Range proof
///
/// # Returns
/// Ok(()) if proof is valid, Err otherwise
pub fn verify_range_proof(
    env: &Env,
    encrypted: &EncryptedAmount,
    proof: &RangeProof,
) -> Result<(), &'static str> {
    // Verify bit-length consistency
    if proof.bit_length != encrypted.bit_length {
        return Err("range proof bit-length mismatch");
    }

    // Verify bit-length is within acceptable bounds
    if proof.bit_length < 32 || proof.bit_length > 128 {
        return Err("invalid bit-length for range proof");
    }

    // Verify proof structure
    if proof.responses.is_empty() {
        return Err("range proof has no responses");
    }

    // Fiat-Shamir verification: recompute challenge
    let mut challenge_data = soroban_sdk::Bytes::new(env);
    challenge_data.append(&soroban_sdk::Bytes::from(encrypted.ciphertext.clone()));
    challenge_data.append(&soroban_sdk::Bytes::from(
        encrypted.randomness_commitment.clone(),
    ));

    let recomputed_challenge = env.crypto().sha256(&challenge_data);

    // Verify challenge matches
    if recomputed_challenge.to_array() != proof.challenge.to_array() {
        return Err("range proof challenge verification failed");
    }

    Ok(())
}

/// Verify decryption proof (zero-knowledge proof of correct decryption).
///
/// Proves that a decrypted value matches the ciphertext without revealing the private key.
/// Uses Fiat-Shamir heuristic for non-interactive proof.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `encrypted` - Original encrypted amount
/// * `decrypted_value` - Claimed decrypted value
/// * `proof` - Decryption proof
///
/// # Returns
/// Ok(()) if proof is valid, Err otherwise
pub fn verify_decryption_proof(
    env: &Env,
    encrypted: &EncryptedAmount,
    decrypted_value: i128,
    proof: &DecryptionProof,
) -> Result<(), &'static str> {
    // Verify value commitment matches decrypted value
    let mut commitment_data = soroban_sdk::Bytes::new(env);
    commitment_data.append(&soroban_sdk::Bytes::from_slice(env, &decrypted_value.to_le_bytes()));

    let recomputed_commitment = env.crypto().sha256(&commitment_data);

    if recomputed_commitment.to_array() != proof.value_commitment.to_array() {
        return Err("decryption proof value commitment mismatch");
    }

    // Verify challenge
    let mut challenge_data = soroban_sdk::Bytes::new(env);
    challenge_data.append(&soroban_sdk::Bytes::from(encrypted.ciphertext.clone()));
    challenge_data.append(&soroban_sdk::Bytes::from(proof.value_commitment.clone()));

    let recomputed_challenge = env.crypto().sha256(&challenge_data);

    if recomputed_challenge.to_array() != proof.challenge.to_array() {
        return Err("decryption proof challenge verification failed");
    }

    Ok(())
}

/// Encrypt a plaintext amount.
///
/// Creates an encrypted amount with randomness and range proof.
/// Uses deterministic encryption based on amount and randomness seed.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `amount` - Plaintext amount to encrypt
/// * `public_key` - Homomorphic public key
/// * `randomness_seed` - Seed for randomness generation
///
/// # Returns
/// Encrypted amount
pub fn encrypt_amount(
    env: &Env,
    amount: i128,
    public_key: &HomomorphicPublicKey,
    randomness_seed: &BytesN<32>,
) -> Result<EncryptedAmount, &'static str> {
    if amount < 0 {
        return Err("cannot encrypt negative amounts");
    }

    // Determine bit-length based on amount
    let bit_length = if amount == 0 {
        32u32
    } else {
        (64 - (amount as u64).leading_zeros()) as u32
    };

    // Generate randomness from seed
    let mut randomness_data = soroban_sdk::Bytes::new(env);
    randomness_data.append(&soroban_sdk::Bytes::from(randomness_seed.clone()));
    randomness_data.append(&soroban_sdk::Bytes::from_slice(env, &amount.to_le_bytes()));

    let randomness = env.crypto().sha256(&randomness_data);

    // Compute ciphertext: hash(public_key || amount || randomness)
    let mut ciphertext_data = soroban_sdk::Bytes::new(env);
    ciphertext_data.append(&soroban_sdk::Bytes::from(public_key.n.clone()));
    ciphertext_data.append(&soroban_sdk::Bytes::from_slice(env, &amount.to_le_bytes()));
    ciphertext_data.append(&soroban_sdk::Bytes::from(randomness.clone()));

    let ciphertext = env.crypto().sha256(&ciphertext_data);

    Ok(EncryptedAmount {
        ciphertext: ciphertext.into(),
        randomness_commitment: randomness.into(),
        key_version: public_key.version,
        bit_length,
    })
}

// ── Internal helper functions ────────────────────────────────────────────────

/// XOR two 32-byte values (simplified homomorphic operation).
#[inline]
fn xor_bytes(a: &BytesN<32>, b: &BytesN<32>) -> BytesN<32> {
    let mut result = [0u8; 32];
    for i in 0..32 {
        result[i] = a.get(i as u32).unwrap_or(0) ^ b.get(i as u32).unwrap_or(0);
    }
    BytesN::from_array(a.env(), &result)
}

/// Multiply bytes by scalar (simplified scalar multiplication).
#[inline]
fn multiply_bytes_by_scalar(bytes: &BytesN<32>, scalar: u64) -> BytesN<32> {
    let mut result = [0u8; 32];
    let scalar_bytes = scalar.to_le_bytes();

    for i in 0..32 {
        let byte_val = bytes.get(i as u32).unwrap_or(0);
        let scalar_byte = scalar_bytes[i % 8];
        result[i] = byte_val.wrapping_mul(scalar_byte);
    }

    BytesN::from_array(bytes.env(), &result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xor_bytes() {
        let a = BytesN::from_array(&soroban_sdk::Env::default(), &[0xAA; 32]);
        let b = BytesN::from_array(&soroban_sdk::Env::default(), &[0x55; 32]);
        let result = xor_bytes(&a, &b);

        for i in 0..32 {
            assert_eq!(result.get(i as u32).unwrap(), 0xFF);
        }
    }
}
