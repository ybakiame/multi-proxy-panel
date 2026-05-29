use base64::Engine;
use rand::RngCore;
use x25519_dalek::{PublicKey, StaticSecret};

/// Generate a cryptographically secure random token string (base64, 32 bytes raw → 43 chars).
pub fn generate_secure_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::STANDARD.encode(&bytes)
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
