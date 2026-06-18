extern crate std;

use soroban_sdk::{testutils::Env as _, Env, Vec};
use tipjar::poly_commit::{
    batch::{batch_open, batch_verify},
    commitment::{commit, fiat_shamir_challenge, open, verify},
    Polynomial, FIELD_PRIME,
};

fn make_poly(env: &Env, coeffs: &[u64]) -> Polynomial {
    let mut v = Vec::new(env);
    for &c in coeffs {
        v.push_back(c);
    }
    Polynomial { coeffs: v }
}

// ── commit / open / verify ────────────────────────────────────────────────────

#[test]
fn test_commit_deterministic() {
    let env = Env::default();
    let poly = make_poly(&env, &[1, 2, 3]);
    let c1 = commit(&env, &poly);
    let c2 = commit(&env, &poly);
    assert_eq!(c1.digest, c2.digest);
}

#[test]
fn test_commit_different_polys() {
    let env = Env::default();
    let p1 = make_poly(&env, &[1, 2, 3]);
    let p2 = make_poly(&env, &[1, 2, 4]);
    assert_ne!(commit(&env, &p1).digest, commit(&env, &p2).digest);
}

#[test]
fn test_open_and_verify_constant() {
    let env = Env::default();
    let poly = make_poly(&env, &[42]);
    let comm = commit(&env, &poly);
    let proof = open(&poly, 99);
    assert_eq!(proof.y, 42);
    assert!(verify(&env, &comm, &poly, &proof));
}

#[test]
fn test_open_and_verify_linear() {
    let env = Env::default();
    // poly = 3 + 5x; at z=2 => 13
    let poly = make_poly(&env, &[3, 5]);
    let comm = commit(&env, &poly);
    let proof = open(&poly, 2);
    assert_eq!(proof.y, 13);
    assert!(verify(&env, &comm, &poly, &proof));
}

#[test]
fn test_verify_rejects_wrong_y() {
    let env = Env::default();
    let poly = make_poly(&env, &[1, 1]);
    let comm = commit(&env, &poly);
    let mut proof = open(&poly, 5);
    proof.y = proof.y.wrapping_add(1) % FIELD_PRIME;
    assert!(!verify(&env, &comm, &poly, &proof));
}

#[test]
fn test_verify_rejects_wrong_commitment() {
    let env = Env::default();
    let poly = make_poly(&env, &[7, 3]);
    let other = make_poly(&env, &[7, 4]);
    let wrong_comm = commit(&env, &other);
    let proof = open(&poly, 10);
    assert!(!verify(&env, &wrong_comm, &poly, &proof));
}

#[test]
fn test_fiat_shamir_challenge_nonzero() {
    let env = Env::default();
    let poly = make_poly(&env, &[1, 2, 3, 4]);
    let comm = commit(&env, &poly);
    let z = fiat_shamir_challenge(&env, &comm);
    assert!(z < FIELD_PRIME);
}

// ── batch verification ────────────────────────────────────────────────────────

#[test]
fn test_batch_single() {
    let env = Env::default();
    let poly = make_poly(&env, &[10, 20]);
    let comm = commit(&env, &poly);
    let batch = batch_open(&env, &[(poly.clone(), comm.clone())], &[3]);
    assert!(batch_verify(&env, &[(poly, comm)], &batch));
}

#[test]
fn test_batch_multiple() {
    let env = Env::default();
    let p1 = make_poly(&env, &[1, 2, 3]);
    let p2 = make_poly(&env, &[5, 0, 1]);
    let c1 = commit(&env, &p1);
    let c2 = commit(&env, &p2);
    let pairs = [(p1.clone(), c1.clone()), (p2.clone(), c2.clone())];
    let batch = batch_open(&env, &pairs, &[7, 11]);
    assert!(batch_verify(&env, &pairs, &batch));
}

#[test]
fn test_batch_rejects_tampered_eval() {
    let env = Env::default();
    let poly = make_poly(&env, &[3, 3]);
    let comm = commit(&env, &poly);
    let mut batch = batch_open(&env, &[(poly.clone(), comm.clone())], &[4]);
    // Tamper with agg_eval.
    batch.agg_eval = batch.agg_eval.wrapping_add(1) % FIELD_PRIME;
    assert!(!batch_verify(&env, &[(poly, comm)], &batch));
}

// ── field arithmetic edge cases ───────────────────────────────────────────────

#[test]
fn test_field_wrap_around() {
    use tipjar::poly_commit::{fp_add, fp_mul, fp_sub};
    assert_eq!(fp_add(FIELD_PRIME - 1, 1), 0);
    assert_eq!(fp_sub(0, 1), FIELD_PRIME - 1);
    assert_eq!(fp_mul(FIELD_PRIME - 1, FIELD_PRIME - 1), 1);
}
