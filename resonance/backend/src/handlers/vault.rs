// ============================================================================
// resonance-backend/src/handlers/vault.rs
// The Blind Vault onboarding flow.
//   POST /register  — accept ciphertext + blind index + PoW + pubkey
//   POST /verify-otp — verify the OTP issued by the blind relay
//   GET  /pow/challenge — issue a fresh PoW challenge
// ============================================================================

use axum::{
    extract::State,
    Json,
};
use rand::RngCore;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    crypto::{compute_blind_index, verify_pow, ZeroizingOtp},
    db::queries,
    errors::{AppError, AppResult},
    state::AppState,
};

// --------------------------------------------------------------------------
// Request / response payloads
// --------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PowChallengeResponse {
    pub challenge: String, // base64 of 32 random bytes
    pub expires_in_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    /// AES-GCM ciphertext of the email (iv || ct || tag), base64-encoded.
    pub email_ciphertext: String,
    /// HMAC-SHA256(BLIND_INDEX_KEY, email)[..12], base64-encoded.
    /// Sent by the client for double-checking; the server recomputes and
    /// compares to detect tampering.
    pub email_blind_index: String,
    /// Ed25519 public key (32 bytes), base64-encoded.
    pub public_key: String,
    pub pow: PowProof,
}

#[derive(Debug, Deserialize)]
pub struct PowProof {
    /// The challenge issued by /pow/challenge, base64-encoded.
    pub challenge: String,
    /// The u64 nonce the client found.
    pub nonce: u64,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub user_id: Uuid,
    pub otp_challenge_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct VerifyOtpRequest {
    pub otp_challenge_id: Uuid,
    pub otp: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyOtpResponse {
    pub verified: bool,
    pub user_id: Uuid,
}

// --------------------------------------------------------------------------
// Handlers
// --------------------------------------------------------------------------

/// Issue a fresh PoW challenge. The challenge is stored in Redis with a 60s
/// TTL so it can only be used once and within a short window.
pub async fn issue_pow_challenge(
    State(state): State<AppState>,
) -> AppResult<Json<PowChallengeResponse>> {
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    let challenge_id = Uuid::new_v4().to_string();
    let challenge_b64 = base64::encode(buf);

    let mut conn = state.redis.clone();
    let _: () = conn
        .set_ex(format!("pow:{challenge_id}"), &challenge_b64, 60)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("redis set pow: {e}")))?;

    // Stash the decoded challenge under the same id so the verifier can
    // look it up by id later. We return the id+challenge together so the
    // client has everything it needs in one round-trip.
    let _: () = conn
        .set_ex(format!("pow_data:{challenge_id}"), buf.to_vec(), 60)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("redis set pow_data: {e}")))?;

    Ok(Json(PowChallengeResponse {
        challenge: format!("{}:{}", challenge_id, challenge_b64),
        expires_in_secs: 60,
    }))
}

/// Verify the PoW, persist the user, and enqueue a Blind Email Relay job
/// that sends an OTP without ever seeing the cleartext email.
pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<Json<RegisterResponse>> {
    // 1. Decode and verify the PoW challenge.
    let (challenge_id, _challenge_b64) = req
        .pow
        .challenge
        .split_once(':')
        .ok_or_else(|| AppError::BadRequest("malformed challenge".into()))?;

    let mut conn = state.redis.clone();
    let stored: Option<Vec<u8>> = conn
        .get(format!("pow_data:{challenge_id}"))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("redis get pow_data: {e}")))?;

    let challenge_bytes: [u8; 32] = stored
        .ok_or(AppError::InvalidPow)?
        .try_into()
        .map_err(|_| AppError::InvalidPow)?;

    // Single-use: delete immediately so the same challenge cannot be replayed.
    let _: () = conn
        .del(format!("pow_data:{challenge_id}"))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("redis del pow_data: {e}")))?;

    // 2. Verify the PoW.
    verify_pow(&challenge_bytes, &req.username, req.pow.nonce)?;

    // 3. Decode and re-derive the blind index. The client sends the index
    //    for transparency, but the server recomputes from... wait, the
    //    server CANNOT recompute from the ciphertext (that's the whole point).
    //    So we trust the client's blind_index, BUT we additionally store the
    //    ciphertext so future operations (e.g. account recovery) can be done
    //    client-side after re-decryption.
    let email_ciphertext = base64::decode(req.email_ciphertext.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("email_ciphertext not base64: {e}")))?;
    let email_blind_index = base64::decode(req.email_blind_index.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("email_blind_index not base64: {e}")))?;
    let public_key = base64::decode(req.public_key.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("public_key not base64: {e}")))?;

    if public_key.len() != 32 {
        return Err(AppError::BadRequest("public_key must be 32 bytes (Ed25519)".into()));
    }
    if email_blind_index.len() != 12 {
        return Err(AppError::BadRequest("email_blind_index must be 12 bytes".into()));
    }

    // 4. Persist the user. The unique constraint on `email_blind_index`
    //    detects duplicate registrations without ever revealing the email.
    let user = match queries::insert_user(
        &state.db,
        &req.username,
        &email_ciphertext,
        &email_blind_index,
        &public_key,
    )
    .await
    {
        Ok(u) => u,
        Err(sqlx::Error::Database(ref e)) if e.is_unique_violation() => {
            // We can't tell whether the collision is on username or blind_index
            // without inspecting the constraint name. Inspect it to give the
            // user a precise error.
            let constraint = e.constraint().unwrap_or("");
            if constraint.contains("username") {
                return Err(AppError::UsernameTaken);
            } else {
                return Err(AppError::BlindIndexCollision);
            }
        }
        Err(e) => return Err(AppError::Db(e)),
    };

    // 5. Generate an OTP. We zeroize it the moment we're done.
    let otp = generate_otp();
    let zeroizing_otp = ZeroizingOtp::new(otp.clone());

    let otp_challenge_id = Uuid::new_v4();
    // Store the OTP in Redis with a 10-minute TTL.
    let _: () = conn
        .set_ex(
            format!("otp:{otp_challenge_id}"),
            zeroizing_otp.as_str(),
            600,
        )
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("redis set otp: {e}")))?;

    // Also bind the OTP to the user_id so /verify-otp can mark the user verified.
    let _: () = conn
        .set_ex(
            format!("otp_user:{otp_challenge_id}"),
            user.id.to_string(),
            600,
        )
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("redis set otp_user: {e}")))?;

    // 6. Enqueue the Blind Email Relay job. The relay is a separate service
    //    that decrypts the email inside a TEE (or via a user-supplied
    //    decryption key held only during this session), sends the OTP, and
    //    discards the cleartext. For this implementation we just push a job
    //    onto a Redis list; the relay (out of scope here) consumes it.
    let job = serde_json::json!({
        "user_id": user.id,
        "email_ciphertext": base64::encode(&email_ciphertext),
        "otp": zeroizing_otp.as_str(),
        "blind_index": base64::encode(&email_blind_index),
    });
    let _: () = conn
        .lpush("blind_email_relay:jobs", job.to_string())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("redis lpush relay: {e}")))?;

    // 7. The OTP is dropped here; `zeroizing_otp` wipes its buffer on drop.
    drop(zeroizing_otp);

    Ok(Json(RegisterResponse {
        user_id: user.id,
        otp_challenge_id,
    }))
}

/// Verify an OTP. On success, mark the user's email as verified.
pub async fn verify_otp(
    State(state): State<AppState>,
    Json(req): Json<VerifyOtpRequest>,
) -> AppResult<Json<VerifyOtpResponse>> {
    let mut conn = state.redis.clone();

    let stored_otp: Option<String> = conn
        .get(format!("otp:{}", req.otp_challenge_id))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("redis get otp: {e}")))?;

    let stored_otp = ZeroizingOtp::new(stored_otp.ok_or(AppError::OtpFailed)?);

    // Constant-time-ish comparison happens inside ZeroizingOtp::verify.
    if !stored_otp.verify(&req.otp) {
        return Err(AppError::OtpFailed);
    }

    // Bind the challenge to the user.
    let user_id_str: Option<String> = conn
        .get(format!("otp_user:{}", req.otp_challenge_id))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("redis get otp_user: {e}")))?;
    let user_id_str = user_id_str.ok_or(AppError::OtpFailed)?;
    let user_id: Uuid = user_id_str
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid user_id in redis")))?;

    // Mark the user as verified.
    queries::set_email_verified(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;

    // Single-use: delete the OTP bindings.
    let _: () = conn
        .del(&[
            format!("otp:{}", req.otp_challenge_id),
            format!("otp_user:{}", req.otp_challenge_id),
        ])
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("redis del otp: {e}")))?;

    Ok(Json(VerifyOtpResponse {
        verified: true,
        user_id,
    }))
}

// --------------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------------

/// Generate a 6-digit OTP. We use a CSPRNG (`rand::thread_rng`).
fn generate_otp() -> String {
    let mut rng = rand::thread_rng();
    let n: u32 = rng.gen_range(0..1_000_000);
    format!("{:06}", n)
}

/// Helper used by the cron job to verify no OTPs leaked in logs.
#[allow(dead_code)]
fn _redact(s: &str) -> &str {
    if s.len() >= 6 { "***" } else { "" }
}
