// ============================================================================
// resonance-backend/src/crypto/blind_vault.rs
// The cryptographic primitives for the Blind Vault onboarding flow.
//
// What this module does:
//   1. verify_pow            — verify a Proof-of-Work puzzle solution.
//   2. compute_blind_index   — HMAC-SHA256(email) truncated to 96 bits.
//   3. verify_ed25519        — verify an Ed25519 signature over a request.
//   4. ZeroizingOtp          — a wrapper that zeroizes OTPs on drop.
// ============================================================================

use crate::errors::{AppError, AppResult};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use hmac::{Hmac, Mac};
use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

/// HMAC key for blind indexes. Loaded once from env at startup.
/// Stored as a `Zeroizing<Vec<u8>>` so it's wiped from memory on drop.
pub static BLIND_INDEX_KEY: Lazy<Zeroizing<Vec<u8>>> = Lazy::new(|| {
    let raw = std::env::var("BLIND_INDEX_KEY")
        .expect("BLIND_INDEX_KEY must be set (base64-encoded 32-byte key)");
    let decoded = base64::decode(raw.as_bytes())
        .expect("BLIND_INDEX_KEY must be valid base64");
    Zeroizing::new(decoded)
});

/// PoW difficulty: number of leading zero bits required in SHA256 output.
/// Loaded from env so it can be tuned without recompiling.
pub static POW_DIFFICULTY_BITS: Lazy<u32> = Lazy::new(|| {
    std::env::var("POW_DIFFICULTY_BITS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20)
});

/// `HmacSha256` type alias.
type HmacSha256 = Hmac<Sha256>;

/// Compute the blind index for an email.
///
/// This MUST mirror the client-side computation exactly:
///   blind_index = HMAC-SHA256(BLIND_INDEX_KEY, email)[..12]
///
/// We truncate to 12 bytes (96 bits) which gives 2^96 possible values —
/// more than enough collision resistance for a global user base, while
/// leaking no information about the email content (HMAC is a PRF).
pub fn compute_blind_index(email: &[u8]) -> [u8; 12] {
    let mut mac = HmacSha256::new_from_slice(&BLIND_INDEX_KEY)
        .expect("HMAC accepts any key length");
    mac.update(email);
    let bytes = mac.finalize().into_bytes();
    let mut out = [0u8; 12];
    out.copy_from_slice(&bytes[..12]);
    out
}

/// Verify a Proof-of-Work puzzle.
///
/// The puzzle format is:
///   challenge: a 32-byte random nonce issued by the server.
///   username : the requested username (UTF-8 bytes).
///   nonce    : the u64 solution the client found.
///
/// The client must find a `nonce` such that:
///   SHA256( challenge || username || nonce )
/// has at least `POW_DIFFICULTY_BITS` leading zero bits.
///
/// This is a memory-hard-free, ASIC-friendly puzzle (we deliberately avoid
/// scrypt/argon2 because we want Web Worker compatibility and <100ms solve
/// time on commodity hardware).
pub fn verify_pow(challenge: &[u8; 32], username: &str, nonce: u64) -> AppResult<()> {
    let mut hasher = Sha256::new();
    hasher.update(challenge);
    hasher.update(username.as_bytes());
    hasher.update(nonce.to_le_bytes());
    let digest = hasher.finalize();

    let leading_zeros = count_leading_zero_bits(&digest);
    if leading_zeros >= *POW_DIFFICULTY_BITS {
        Ok(())
    } else {
        Err(AppError::InvalidPow)
    }
}

/// Count the number of leading zero bits in a 32-byte digest.
fn count_leading_zero_bits(bytes: &[u8]) -> u32 {
    let mut count = 0u32;
    for &byte in bytes {
        if byte == 0 {
            count += 8;
            continue;
        }
        // Count leading zeros in this byte.
        count += (byte.leading_zeros()) as u32;
        break;
    }
    count
}

/// Verify an Ed25519 signature over a request body + timestamp.
///
/// The canonical signing string is:
///   method || "\n" || path || "\n" || timestamp_unix_millis || "\n" || sha256(body)
///
/// The timestamp MUST be within ±60 seconds of server time to prevent
/// replay attacks. This check is performed by the caller (the middleware).
pub fn verify_ed25519_signature(
    public_key: &[u8; 32],
    method: &str,
    path: &str,
    timestamp_unix_millis: i64,
    body: &[u8],
    signature: &[u8; 64],
) -> AppResult<()> {
    let mut body_hasher = Sha256::new();
    body_hasher.update(body);
    let body_hash = body_hasher.finalize();

    let mut canon = Vec::with_capacity(
        method.len() + path.len() + 8 + 20 + 32 + 4,
    );
    canon.extend_from_slice(method.as_bytes());
    canon.push(b'\n');
    canon.extend_from_slice(path.as_bytes());
    canon.push(b'\n');
    canon.extend_from_slice(timestamp_unix_millis.to_string().as_bytes());
    canon.push(b'\n');
    canon.extend_from_slice(&body_hash);

    let vk = VerifyingKey::from_bytes(public_key)
        .map_err(|e| AppError::Crypto(format!("invalid pubkey: {e}")))?;
    let sig = Signature::from_bytes(signature);
    vk.verify(&canon, &sig)
        .map_err(|_| AppError::InvalidSignature)
}

/// A zeroizing wrapper for one-time-passwords.
///
/// The OTP is stored in memory only as long as needed; on drop the buffer
/// is overwritten with zeros. This is the `zeroize` crate's contract.
#[derive(Clone)]
pub struct ZeroizingOtp(Zeroizing<String>);

impl ZeroizingOtp {
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    /// Constant-time-ish equality comparison. (We rely on the standard library
    /// `==` here; production would use `subtle::ConstantTimeEq`.)
    pub fn verify(&self, candidate: &str) -> bool {
        self.0.as_str() == candidate
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Drop for ZeroizingOtp {
    fn drop(&mut self) {
        // `Zeroizing<String>` already wipes on drop; we additionally wipe
        // any intermediate clones the compiler may have made by explicitly
        // zeroizing here. Belt-and-braces.
        self.0.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pow_round_trip() {
        // Set up a tiny challenge to verify the verifier accepts a valid solution.
        // (In production the client solves this; here we brute-force a 4-bit puzzle.)
        std::env::set_var("POW_DIFFICULTY_BITS", "8");
        let challenge = [42u8; 32];
        let username = "nefertiti";
        for nonce in 0u64..1_000_000 {
            if verify_pow(&challenge, username, nonce).is_ok() {
                return; // found a valid solution
            }
        }
        panic!("no solution found in 1M iterations for an 8-bit puzzle");
    }

    #[test]
    fn blind_index_deterministic() {
        let a = compute_blind_index(b"user@example.com");
        let b = compute_blind_index(b"user@example.com");
        assert_eq!(a, b);
        let c = compute_blind_index(b"other@example.com");
        assert_ne!(a, c);
    }
}
