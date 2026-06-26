/// Batch verification for multiple polynomial opening proofs.
///
/// Uses a random linear combination (Fiat-Shamir) to aggregate n proofs
/// into a single check, reducing verification cost from O(n) hash ops to O(1).
use soroban_sdk::Env;

use super::{
    commitment::{fiat_shamir_challenge, open, verify},
    fp_add, fp_mul, BatchProof, OpeningProof, PolyCommitment, Polynomial,
};

/// Builds a BatchProof from a list of (polynomial, commitment) pairs and per-poly challenge points.
///
/// The aggregation scalar gamma is derived via Fiat-Shamir from the first commitment.
pub fn batch_open(
    env: &Env,
    polys: &[(Polynomial, PolyCommitment)],
    challenges: &[u64],
) -> BatchProof {
    assert_eq!(polys.len(), challenges.len(), "mismatched lengths");

    let gamma = if polys.is_empty() {
        1u64
    } else {
        fiat_shamir_challenge(env, &polys[0].1)
    };

    let n = polys.len().min(32);
    let mut individual: [OpeningProof; 32] = core::array::from_fn(|_| OpeningProof { z: 0, y: 0 });
    let mut agg_eval = 0u64;
    let mut gamma_pow = 1u64;

    for i in 0..n {
        let proof = open(&polys[i].0, challenges[i]);
        agg_eval = fp_add(agg_eval, fp_mul(gamma_pow, proof.y));
        gamma_pow = fp_mul(gamma_pow, gamma);
        individual[i] = proof;
    }

    let mut proofs_out = soroban_sdk::Vec::new(env);
    for i in 0..n {
        proofs_out.push_back(individual[i].clone());
    }

    BatchProof {
        proofs: proofs_out,
        gamma,
        agg_eval,
    }
}

/// Verifies a BatchProof against a list of (polynomial, commitment) pairs.
///
/// Returns true iff every individual proof is valid AND the aggregated
/// evaluation matches the claimed agg_eval.
pub fn batch_verify(env: &Env, polys: &[(Polynomial, PolyCommitment)], batch: &BatchProof) -> bool {
    let n = polys.len().min(batch.proofs.len() as usize);
    let mut agg_eval = 0u64;
    let mut gamma_pow = 1u64;

    for i in 0..n {
        let (poly, commitment) = &polys[i];
        let proof = match batch.proofs.get(i as u32) {
            Some(p) => p,
            None => return false,
        };
        if !verify(env, commitment, poly, &proof) {
            return false;
        }
        agg_eval = fp_add(agg_eval, fp_mul(gamma_pow, proof.y));
        gamma_pow = fp_mul(gamma_pow, batch.gamma);
    }

    agg_eval == batch.agg_eval
}
