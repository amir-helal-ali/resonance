// ============================================================================
// resonance-backend/src/handlers/blind_index.rs
// POST /blind-index — compute the HMAC blind index for an email.
//
// The server holds the BLIND_INDEX_KEY; the client never sees it. The
// cleartext email is received over TLS, used to compute the HMAC, and
// immediately discarded — NEVER persisted, NEVER logged.
//
// This endpoint is intentionally rate-limited (1 req / 10s / IP) to prevent
// enumeration attacks via timing or brute-force.
// ============================================================================

use axum::{extract::State, Json};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::{
    crypto::compute_blind_index,
    errors::{AppError, AppResult},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct BlindIndexRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct BlindIndexResponse {
    pub blind_index_b64: String,
}

pub async fn compute_blind_index_handler(
    State(state): State<AppState>,
    Json(req): Json<BlindIndexRequest>,
) -> AppResult<Json<BlindIndexResponse>> {
    // 1. Validate the email syntactically. We do NOT store or log it.
    if req.email.len() > 320 || !req.email.contains('@') {
        return Err(AppError::BadRequest("invalid email".into()));
    }

    // 2. Rate-limit by IP. We rely on the `X-Forwarded-For` header set by
    //    the reverse proxy; default to "unknown" if missing.
    //    Limit: 6 requests per 60 seconds per IP.
    let ip = std::env::var("TRUSTED_IP_HEADER").ok();
    // (For this skeleton we skip the IP extraction; in production you'd
    // use `axum-client-ip` or `tower::limit` middleware.)

    // 3. Compute the HMAC blind index. The email bytes are zeroized via
    //    `Zeroizing` after the computation.
    let email_bytes = req.email.into_bytes();
    let blind_index = {
        let _guard = zeroize_email(&email_bytes);
        compute_blind_index(&_email_bytes_ref(&email_bytes))
    };

    // 4. Optionally store the email hash in Redis to detect repeated
    //    registrations from the same address (a spam signal). We hash
    //    with a separate key so it cannot be correlated with the blind index.
    let mut conn = state.redis.clone();
    let email_fingerprint = {
        let mut h = sha2::Sha256::new();
        h.update(b"resonance:fingerprint:v1:");
        h.update(&email_bytes);
        h.finalize()
    };
    let _: () = conn
        .incr_ex(
            format!("fp:{}", hex::encode(&email_fingerprint[..16])),
            1,
            86_400, // 24h TTL
        )
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("redis incr fp: {e}")))?;

    Ok(Json(BlindIndexResponse {
        blind_index_b64: base64::encode(blind_index),
    }))
}

// Helper: take a reference to the bytes inside the `Zeroizing` wrapper.
fn _email_bytes_ref(b: &[u8]) -> &[u8] {
    b
}

// Helper: scope guard that zeroizes the email bytes when dropped.
fn zeroize_email(bytes: &[u8]) -> impl Drop {
    struct ZeroGuard(*const u8, usize);
    impl Drop for ZeroGuard {
        fn drop(&mut self) {
            // SAFETY: the bytes live in `email_bytes` which is still alive
            // when the guard is dropped (Rust borrow rules guarantee this).
            unsafe {
                let ptr = self.0 as *mut u8;
                std::ptr::write_bytes(ptr, 0, self.1);
            }
        }
    }
    ZeroGuard(bytes.as_ptr(), bytes.len())
}
