pub mod batch;
/// Polynomial commitment scheme for tip data verification.
///
/// Uses a hash-based univariate polynomial commitment over F_p where
/// p = 2^61 - 1 (Mersenne prime). Commitment = SHA256(coefficients).
/// Opening proof = polynomial evaluation at a challenge point.
pub mod commitment;

use soroban_sdk::{contracttype, BytesN, Vec};

/// Prime modulus: 2^61 - 1 (Mersenne prime).
pub const FIELD_PRIME: u64 = (1u64 << 61) - 1;

/// A polynomial over F_p represented by its coefficients [c0, c1, ..., cn]
/// where poly(x) = c0 + c1*x + c2*x^2 + ... + cn*x^n.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Polynomial {
    /// Coefficients in F_p, index = degree.
    pub coeffs: Vec<u64>,
}

/// A polynomial commitment: SHA256 of the serialized coefficients.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolyCommitment {
    pub digest: BytesN<32>,
}

/// An opening proof: the evaluation y = poly(z) at challenge z.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpeningProof {
    /// Challenge point z in F_p.
    pub z: u64,
    /// Claimed evaluation y = poly(z) in F_p.
    pub y: u64,
}

/// A batch opening proof for multiple polynomials at multiple points.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchProof {
    /// Per-polynomial opening proofs.
    pub proofs: Vec<OpeningProof>,
    /// Aggregated challenge (random linear combination scalar).
    pub gamma: u64,
    /// Aggregated evaluation: sum_i gamma^i * y_i.
    pub agg_eval: u64,
}

// ── Field arithmetic over F_p (p = 2^61 - 1) ────────────────────────────────

/// Reduces x modulo FIELD_PRIME using the Mersenne structure.
#[inline]
pub fn fp_reduce(x: u128) -> u64 {
    // For p = 2^61 - 1: x mod p = (x >> 61) + (x & p), then one conditional sub.
    let lo = (x & (FIELD_PRIME as u128)) as u64;
    let hi = (x >> 61) as u64;
    let sum = lo + hi;
    if sum >= FIELD_PRIME {
        sum - FIELD_PRIME
    } else {
        sum
    }
}

/// Addition in F_p.
#[inline]
pub fn fp_add(a: u64, b: u64) -> u64 {
    let s = a + b;
    if s >= FIELD_PRIME {
        s - FIELD_PRIME
    } else {
        s
    }
}

/// Subtraction in F_p.
#[inline]
pub fn fp_sub(a: u64, b: u64) -> u64 {
    if a >= b {
        a - b
    } else {
        a + FIELD_PRIME - b
    }
}

/// Multiplication in F_p.
#[inline]
pub fn fp_mul(a: u64, b: u64) -> u64 {
    fp_reduce((a as u128) * (b as u128))
}

/// Exponentiation in F_p via square-and-multiply.
pub fn fp_pow(mut base: u64, mut exp: u64) -> u64 {
    let mut result = 1u64;
    base %= FIELD_PRIME;
    while exp > 0 {
        if exp & 1 == 1 {
            result = fp_mul(result, base);
        }
        base = fp_mul(base, base);
        exp >>= 1;
    }
    result
}

/// Evaluates poly at point z using Horner's method.
pub fn poly_eval(coeffs: &[u64], z: u64) -> u64 {
    if coeffs.is_empty() {
        return 0;
    }
    let mut result = *coeffs.last().unwrap();
    for &c in coeffs.iter().rev().skip(1) {
        result = fp_add(fp_mul(result, z), c);
    }
    result
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn test_fp_add_wrap() {
        assert_eq!(fp_add(FIELD_PRIME - 1, 1), 0);
    }

    #[test]
    fn test_fp_mul() {
        assert_eq!(fp_mul(2, 3), 6);
        // (p-1)^2 mod p = 1
        assert_eq!(fp_mul(FIELD_PRIME - 1, FIELD_PRIME - 1), 1);
    }

    #[test]
    fn test_fp_pow() {
        assert_eq!(fp_pow(2, 10), 1024);
        assert_eq!(fp_pow(0, 0), 1);
    }

    #[test]
    fn test_poly_eval_constant() {
        // poly = [5] => eval at any z = 5
        assert_eq!(poly_eval(&[5], 7), 5);
    }

    #[test]
    fn test_poly_eval_linear() {
        // poly = [1, 2] => 1 + 2z; at z=3 => 7
        assert_eq!(poly_eval(&[1, 2], 3), 7);
    }

    #[test]
    fn test_poly_eval_quadratic() {
        // poly = [0, 0, 1] => z^2; at z=4 => 16
        assert_eq!(poly_eval(&[0, 0, 1], 4), 16);
    }
}
