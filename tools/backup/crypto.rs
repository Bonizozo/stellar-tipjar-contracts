//! AES-256-GCM encryption/decryption helpers for backup files.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};

/// Encrypt `plaintext` with AES-256-GCM using `key`.
/// Output format: base64(nonce || ciphertext).
pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce_bytes: [u8; 12] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);

    Ok(B64.encode(&combined).into_bytes())
}

/// Decrypt a backup file produced by [`encrypt`].
pub fn decrypt(encoded: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    let combined = B64
        .decode(encoded)
        .context("Failed to base64-decode encrypted backup")?;

    anyhow::ensure!(combined.len() > 12, "Ciphertext too short");

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed (wrong key?): {}", e))
}
