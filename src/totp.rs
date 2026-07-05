//! TOTP helpers (RFC 6238) for authenticator-app 2FA.

use anyhow::{Result, anyhow};
use rand::RngCore;
use totp_rs::{Algorithm, Rfc6238, Secret, TOTP};

const ISSUER: &str = "Statbook.io";
const DIGITS: usize = 6;
const STEP_SECONDS: u64 = 30;
const SKEW_STEPS: u8 = 1;

fn secret_from_base32(secret_b32: &str) -> Result<Secret> {
    Ok(Secret::Encoded(secret_b32.to_string()))
}

fn totp_from_base32(secret_b32: &str, account: &str) -> Result<TOTP> {
    let secret = secret_from_base32(secret_b32)?;
    TOTP::new(
        Algorithm::SHA1,
        DIGITS,
        SKEW_STEPS,
        STEP_SECONDS,
        secret.to_bytes().map_err(|e| anyhow!("invalid TOTP secret: {e}"))?,
        Some(ISSUER.to_string()),
        account.to_string()
    )
    .map_err(|e| anyhow!("TOTP init failed: {e}"))
}

/// Generate a new base32-encoded TOTP secret suitable for authenticator apps.
pub fn generate_secret() -> Result<String> {
    let mut raw = [0u8; 20];
    rand::rng().fill_bytes(&mut raw);
    let secret = Secret::Raw(raw.to_vec());
    Ok(secret.to_encoded().to_string())
}

/// Build an `otpauth://` URI for QR encoding.
pub fn otpauth_uri(account: &str, secret_b32: &str) -> Result<String> {
    let secret = secret_from_base32(secret_b32)?;
    let bytes = secret.to_bytes().map_err(|e| anyhow!("invalid TOTP secret: {e}"))?;
    let rfc = Rfc6238::new(DIGITS, bytes, Some(ISSUER.to_string()), account.to_string())
        .map_err(|e| anyhow!("RFC6238 init failed: {e}"))?;
    let totp = TOTP::from_rfc6238(rfc).map_err(|e| anyhow!("TOTP from RFC6238 failed: {e}"))?;
    Ok(totp.get_url())
}

/// Verify a 6-digit TOTP code against a base32 secret (±1 step window).
pub fn verify_code(secret_b32: &str, code: &str) -> Result<bool> {
    let trimmed = code.trim();
    if trimmed.len() != DIGITS || !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Ok(false);
    }

    let totp = totp_from_base32(secret_b32, "")?;
    totp.check_current(trimmed).map_err(|e| anyhow!("TOTP verify failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_secret_produces_non_empty_base32() {
        let secret = generate_secret().expect("generate");
        assert!(!secret.is_empty());
        assert!(secret.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn otpauth_uri_contains_issuer_and_account() {
        let secret = generate_secret().expect("generate");
        let uri = otpauth_uri("alice@example.com", &secret).expect("uri");
        assert!(uri.starts_with("otpauth://"));
        assert!(uri.contains("Statbook.io"));
    }

    #[test]
    fn verify_current_code_round_trip() {
        let secret_b32 = generate_secret().expect("generate");
        let totp = totp_from_base32(&secret_b32, "test@example.com").expect("totp");
        let code = totp.generate_current().expect("code");
        assert!(verify_code(&secret_b32, &code).expect("verify"));
    }

    #[test]
    fn verify_rejects_invalid_code() {
        let secret_b32 = generate_secret().expect("generate");
        assert!(!verify_code(&secret_b32, "000000").expect("verify"));
    }
}
