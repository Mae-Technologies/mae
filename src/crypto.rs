//! Symmetric encryption for secrets at rest (AES-256-GCM).
//!
//! Random nonce is prepended to ciphertext, then base64-encoded for TEXT storage.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce
};
use rand::RngCore;
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use secrecy::{ExposeSecret, SecretString};

const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

fn key_bytes(secret: &SecretString) -> [u8; KEY_LEN] {
    let exposed = secret.expose_secret();
    let mut key = [0u8; KEY_LEN];
    let bytes = exposed.as_bytes();
    let len = bytes.len().min(KEY_LEN);
    key[..len].copy_from_slice(&bytes[..len]);
    key
}

/// Encrypt a plaintext secret for database storage.
pub fn encrypt_field(encryption_key: &SecretString, plaintext: &str) -> Result<String> {
    if plaintext.is_empty() {
        return Ok(String::new());
    }

    let cipher = Aes256Gcm::new_from_slice(&key_bytes(encryption_key))
        .map_err(|e| anyhow!("invalid encryption key: {e}"))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow!("encryption failed: {e}"))?;

    let mut payload = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    payload.extend_from_slice(&nonce_bytes);
    payload.extend_from_slice(&ciphertext);

    Ok(STANDARD.encode(payload))
}

/// Decrypt a stored ciphertext field. Returns `None` when the column is empty.
pub fn decrypt_field(encryption_key: &SecretString, stored: &str) -> Result<Option<String>> {
    if stored.is_empty() {
        return Ok(None);
    }

    let payload = STANDARD.decode(stored).context("invalid base64 ciphertext")?;

    if payload.len() <= NONCE_LEN {
        return Err(anyhow!("ciphertext too short"));
    }

    let (nonce_bytes, ciphertext) = payload.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes(encryption_key))
        .map_err(|e| anyhow!("invalid encryption key: {e}"))?;
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext =
        cipher.decrypt(nonce, ciphertext).map_err(|e| anyhow!("decryption failed: {e}"))?;

    String::from_utf8(plaintext).context("decrypted value is not valid utf-8").map(Some)
}

/// Admin peek: decrypt AES-GCM ciphertext, or return legacy plaintext as-is.
pub fn peek_field(encryption_key: &SecretString, stored: &str) -> Result<Option<String>> {
    if stored.is_empty() {
        return Ok(None);
    }
    match decrypt_field(encryption_key, stored) {
        Ok(value) => Ok(value),
        Err(_) => Ok(Some(stored.to_string()))
    }
}