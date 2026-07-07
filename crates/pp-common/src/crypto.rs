use base64::Engine;
use rand::RngCore;
use x25519_dalek::{PublicKey, StaticSecret};

/// Generate a cryptographically secure random token string (base64, 32 bytes raw → 43 chars).
pub fn generate_secure_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Generate a random UUID v4 string.
pub fn generate_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Generate an X25519 keypair for REALITY.
/// Returns (private_key_base64, public_key_base64).
pub fn generate_x25519_keypair() -> (String, String) {
    let secret = StaticSecret::random();
    let public = PublicKey::from(&secret);
    let private_b64 = base64::engine::general_purpose::STANDARD.encode(secret.as_bytes());
    let public_b64 = base64::engine::general_purpose::STANDARD.encode(public.as_bytes());
    (private_b64, public_b64)
}

/// Generate a random short_id for REALITY (8 hex chars).
pub fn generate_short_id() -> String {
    let mut bytes = [0u8; 4];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

const ARGON2_M_COST: u32 = 64 * 1024;
const ARGON2_T_COST: u32 = 3;
const ARGON2_P_COST: u32 = 4;

/// Hash a secret token (API key or agent token) using Argon2id.
/// The returned string includes the encoded salt and parameters.
pub fn hash_secret(secret: &str) -> Result<String, argon2::password_hash::Error> {
    use argon2::password_hash::SaltString;
    use argon2::password_hash::rand_core::OsRng;
    use argon2::{Argon2, PasswordHasher};

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, None)?,
    );
    Ok(argon2.hash_password(secret.as_bytes(), &salt)?.to_string())
}

/// Verify a secret against an Argon2id hash.
pub fn verify_secret(secret: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};

    let parsed = PasswordHash::new(hash)?;
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, None)?,
    );
    Ok(argon2.verify_password(secret.as_bytes(), &parsed).is_ok())
}

/// Constant-time equality comparison for two equal-length byte strings.
pub fn secure_eq(a: &[u8], b: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    a.ct_eq(b).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_hash_verify_roundtrip() {
        let secret = "ck_test_secret_value";
        let hash = hash_secret(secret).expect("hash failed");
        assert!(!hash.is_empty());
        assert!(verify_secret(secret, &hash).expect("verify failed"));
        assert!(!verify_secret("wrong", &hash).expect("verify failed"));
    }

    #[test]
    fn secure_eq_works() {
        assert!(secure_eq(b"abc", b"abc"));
        assert!(!secure_eq(b"abc", b"ab"));
        assert!(!secure_eq(b"abc", b"abd"));
    }
}
