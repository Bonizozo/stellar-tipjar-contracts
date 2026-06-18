use soroban_sdk::{Bytes, Env};

use super::{fp_add, fp_mul, fp_sub, poly_eval, OpeningProof, PolyCommitment, Polynomial};

/// Commits to a polynomial by hashing its serialized coefficients.
///
/// commitment = SHA256(c0_le || c1_le || ... || cn_le)
pub fn commit(env: &Env, poly: &Polynomial) -> PolyCommitment {
    PolyCommitment {
        digest: env.crypto().sha256(&serialize_coeffs(env, poly)).into(),
    }
}

/// Produces an opening proof: evaluates poly at challenge z.
pub fn open(poly: &Polynomial, z: u64) -> OpeningProof {
    let len = poly.coeffs.len() as usize;
    let mut buf = [0u64; 64];
    for i in 0..len.min(64) {
        buf[i] = poly.coeffs.get(i as u32).unwrap_or(0);
    }
    OpeningProof {
        z,
        y: poly_eval(&buf[..len.min(64)], z),
    }
}

/// Verifies an opening proof against a commitment.
///
/// Re-hashes the polynomial to check the commitment, then re-evaluates at z.
pub fn verify(
    env: &Env,
    commitment: &PolyCommitment,
    poly: &Polynomial,
    proof: &OpeningProof,
) -> bool {
    commit(env, poly).digest == commitment.digest && {
        let len = poly.coeffs.len() as usize;
        let mut buf = [0u64; 64];
        for i in 0..len.min(64) {
            buf[i] = poly.coeffs.get(i as u32).unwrap_or(0);
        }
        poly_eval(&buf[..len.min(64)], proof.z) == proof.y
    }
}

/// Derives a Fiat-Shamir challenge from a commitment.
///
/// Returns a field element in F_p from the first 8 bytes of SHA256(digest).
pub fn fiat_shamir_challenge(env: &Env, commitment: &PolyCommitment) -> u64 {
    let h = env.crypto().sha256(&Bytes::from(&commitment.digest));
    let mut z = 0u64;
    for i in 0..8u32 {
        z |= (h.to_array()[i as usize] as u64) << (8 * i);
    }
    super::fp_reduce(z as u128)
}

/// Computes the quotient polynomial q(x) = (poly(x) - y) / (x - z) via synthetic division.
///
/// Returns coefficients of q as a soroban Vec (same length as poly, last element is 0).
pub fn quotient_poly(env: &Env, poly: &Polynomial, z: u64, y: u64) -> soroban_sdk::Vec<u64> {
    let n = poly.coeffs.len() as usize;
    let mut c = [0u64; 64];
    for i in 0..n.min(64) {
        c[i] = poly.coeffs.get(i as u32).unwrap_or(0);
    }
    // Subtract y from constant term.
    c[0] = fp_sub(c[0], y);

    // Synthetic division by (x - z): process high-to-low.
    let mut q = [0u64; 64];
    let mut rem = 0u64;
    for i in (0..n.min(64)).rev() {
        let cur = fp_add(c[i], rem);
        if i > 0 {
            q[i - 1] = cur;
            rem = fp_mul(cur, z);
        }
    }

    let mut out = soroban_sdk::Vec::new(env);
    let q_len = if n > 1 { n - 1 } else { 1 };
    for i in 0..q_len {
        out.push_back(q[i]);
    }
    out
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn serialize_coeffs(env: &Env, poly: &Polynomial) -> Bytes {
    let mut data = Bytes::new(env);
    for i in 0..poly.coeffs.len() {
        let c = poly.coeffs.get(i).unwrap_or(0);
        data.append(&Bytes::from_array(env, &c.to_le_bytes()));
    }
    data
}
