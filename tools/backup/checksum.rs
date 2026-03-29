//! SHA-256 checksum helpers for backup integrity verification.

use sha2::{Digest, Sha256};

/// Return the lowercase hex SHA-256 digest of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Verify that `data` matches `expected_hex` checksum.
pub fn verify(data: &[u8], expected_hex: &str) -> bool {
    sha256_hex(data) == expected_hex
}
