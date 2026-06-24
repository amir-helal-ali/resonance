// ============================================================================
// resonance-backend/tests/crypto_spec.rs
// Integration tests for the Blind Vault cryptographic primitives.
// ============================================================================

use resonance_backend::crypto::{
    compute_blind_index, verify_ed25519_signature, verify_pow, ZeroizingOtp,
};
use ed25519_dalek::{Signature, Signer, SigningKey};

#[test]
fn blind_index_is_deterministic_and_distinct() {
    let a = compute_blind_index(b"nefertiti@nile.eg");
    let b = compute_blind_index(b"nefertiti@nile.eg");
    let c = compute_blind_index(b"akhenaten@nile.eg");
    assert_eq!(a, b, "same email must produce same index");
    assert_ne!(a, c, "different emails must produce different indexes");
}

#[test]
fn blind_index_is_12_bytes() {
    let idx = compute_blind_index(b"test@example.com");
    assert_eq!(idx.len(), 12, "blind index must be 96 bits");
}

#[test]
fn pow_accepts_valid_solution() {
    std::env::set_var("POW_DIFFICULTY_BITS", "8"); // 8 bits = trivially solvable
    let challenge = [0xab; 32];
    let username = "test_user";
    // Brute-force a valid nonce.
    for nonce in 0u64..1_000_000 {
        if verify_pow(&challenge, username, nonce).is_ok() {
            return;
        }
    }
    panic!("no valid PoW found in 1M iterations for an 8-bit puzzle");
}

#[test]
fn pow_rejects_invalid_solution() {
    std::env::set_var("POW_DIFFICULTY_BITS", "32"); // hard
    let challenge = [0xcd; 32];
    let username = "test_user";
    // Nonce 0 will not satisfy 32 bits of leading zeros.
    assert!(verify_pow(&challenge, username, 0).is_err());
}

#[test]
fn ed25519_signature_round_trip() {
    // Generate a keypair.
    let mut csprng = rand::thread_rng();
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    let pubkey_bytes = verifying_key.to_bytes();

    // Build a canonical message.
    let method = "POST";
    let path = "/pulses";
    let ts: i64 = 1_700_000_000_000;
    let body = br#"{"hello":"world"}"#;

    // Hash the body the same way the verifier does.
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(body);
    let body_hash = h.finalize();

    let mut canon = Vec::new();
    canon.extend_from_slice(method.as_bytes());
    canon.push(b'\n');
    canon.extend_from_slice(path.as_bytes());
    canon.push(b'\n');
    canon.extend_from_slice(ts.to_string().as_bytes());
    canon.push(b'\n');
    canon.extend_from_slice(&body_hash);

    let sig: Signature = signing_key.sign(&canon);
    let sig_bytes = sig.to_bytes();

    assert!(
        verify_ed25519_signature(&pubkey_bytes, method, path, ts, body, &sig_bytes).is_ok(),
        "valid signature must verify"
    );
}

#[test]
fn ed25519_signature_rejects_tampered_body() {
    let mut csprng = rand::thread_rng();
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    let pubkey_bytes = verifying_key.to_bytes();

    let method = "POST";
    let path = "/pulses";
    let ts: i64 = 1_700_000_000_000;
    let body = br#"original"#;
    let tampered = br#"tampered"#;

    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(body);
    let body_hash = h.finalize();
    let mut canon = Vec::new();
    canon.extend_from_slice(method.as_bytes());
    canon.push(b'\n');
    canon.extend_from_slice(path.as_bytes());
    canon.push(b'\n');
    canon.extend_from_slice(ts.to_string().as_bytes());
    canon.push(b'\n');
    canon.extend_from_slice(&body_hash);

    let sig: Signature = signing_key.sign(&canon);
    let sig_bytes = sig.to_bytes();

    assert!(
        verify_ed25519_signature(&pubkey_bytes, method, path, ts, tampered, &sig_bytes).is_err(),
        "tampered body must fail verification"
    );
}

#[test]
fn zeroizing_otp_verify_works() {
    let otp = ZeroizingOtp::new("123456".into());
    assert!(otp.verify("123456"));
    assert!(!otp.verify("000000"));
}

#[test]
fn zeroizing_otp_drops_safely() {
    // We can't directly assert zeroization in safe Rust, but we can at least
    // confirm that Drop runs without panicking.
    {
        let _otp = ZeroizingOtp::new("secret".into());
    }
    // If we reach here, drop ran cleanly.
}
