// ============================================================================
// resonance-backend/src/crypto/mod.rs
// ============================================================================

pub mod blind_vault;

pub use blind_vault::{
    compute_blind_index, verify_ed25519_signature, verify_pow, ZeroizingOtp,
    BLIND_INDEX_KEY, POW_DIFFICULTY_BITS,
};
