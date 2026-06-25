// ============================================================================
// resonance-backend/src/handlers/settings.rs
// Account settings: update imprint/horizon, rotate public key, delete account.
// ============================================================================

use axum::{
    extract::State,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    errors::{AppError, AppResult},
    middleware::signature::VerifiedPubKey,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub imprint: Option<String>,
    pub horizon: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateProfileResponse {
    pub updated: Vec<String>,
}

/// PATCH /settings/profile — update imprint and/or horizon.
pub async fn update_profile(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Json(req): Json<UpdateProfileRequest>,
) -> AppResult<Json<UpdateProfileResponse>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let mut updated = Vec::new();
    if let Some(ref imprint) = req.imprint {
        if imprint.len() > 500 {
            return Err(AppError::BadRequest("imprint max 500 chars".into()));
        }
        sqlx::query("UPDATE users SET imprint = $1 WHERE id = $2")
            .bind(imprint)
            .bind(me.id)
            .execute(&state.db)
            .await
            .map_err(AppError::Db)?;
        updated.push("imprint".into());
    }
    if let Some(ref horizon) = req.horizon {
        if horizon.len() > 500 {
            return Err(AppError::BadRequest("horizon max 500 chars".into()));
        }
        sqlx::query("UPDATE users SET horizon = $1 WHERE id = $2")
            .bind(horizon)
            .bind(me.id)
            .execute(&state.db)
            .await
            .map_err(AppError::Db)?;
        updated.push("horizon".into());
    }

    Ok(Json(UpdateProfileResponse { updated }))
}

#[derive(Debug, Deserialize)]
pub struct RotateKeyRequest {
    /// The new Ed25519 public key (32 bytes), base64-encoded.
    pub new_public_key_b64: String,
    /// A signature over "rotate-key" || old_pubkey || new_pubkey, signed by
    /// the OLD private key. This proves the rotation is authorized by the
    /// current key holder (not an attacker who stole the new key).
    pub authorization_sig_b64: String,
}

/// POST /settings/rotate-key — rotate the Ed25519 public key.
/// Requires a signature from the OLD key to authorize the rotation.
pub async fn rotate_key(
    State(state): State<AppState>,
    VerifiedPubKey(old_pubkey): VerifiedPubKey,
    Json(req): Json<RotateKeyRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &old_pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let new_pubkey = base64::decode(req.new_public_key_b64.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("new_public_key not base64: {e}")))?;
    if new_pubkey.len() != 32 {
        return Err(AppError::BadRequest("new_public_key must be 32 bytes".into()));
    }

    // Verify the authorization signature: old_key signs (old || new).
    let canon = [old_pubkey.as_ref(), &new_pubkey].concat();
    let sig = base64::decode(req.authorization_sig_b64.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("authorization_sig not base64: {e}")))?;
    if sig.len() != 64 {
        return Err(AppError::BadRequest("authorization_sig must be 64 bytes".into()));
    }

    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    let vk = VerifyingKey::from_bytes(&old_pubkey)
        .map_err(|e| AppError::Crypto(format!("invalid old pubkey: {e}")))?;
    let signature = Signature::from_bytes(
        sig.as_slice().try_into().map_err(|_| AppError::InvalidSignature)?,
    );
    vk.verify(&canon, &signature).map_err(|_| AppError::InvalidSignature)?;

    // Update the user's public key.
    sqlx::query("UPDATE users SET public_key = $1 WHERE id = $2")
        .bind(&new_pubkey)
        .bind(me.id)
        .execute(&state.db)
        .await
        .map_err(AppError::Db)?;

    tracing::info!(user_id = %me.id, "public key rotated");

    Ok(Json(serde_json::json!({
        "rotated": true,
        "new_public_key_b64": req.new_public_key_b64,
    })))
}

/// DELETE /settings/account — permanently delete the account.
/// This cascades to: pulses, connections, traces, DMs, notifications, goals.
/// The user's Ed25519 private key should also be wiped from IndexedDB
/// client-side (the frontend handles this after the 200 response).
pub async fn delete_account(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
) -> AppResult<Json<serde_json::Value>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    // The CASCADE on `users.id` will clean up:
    //   - pulses (→ encryption_keys via pulses.encryption_key_id, but those
    //     have ON DELETE NO ACTION — we need to evaporate first).
    //   - connections, traces, DMs, notifications, goals, goal_candles, etc.

    // 1. Evaporate all my pulses (destroy keys first).
    let pulse_ids: Vec<(uuid::Uuid,)> = sqlx::query_as("SELECT id FROM pulses WHERE author_id = $1")
        .bind(me.id)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Db)?;
    for (pid,) in pulse_ids {
        let _ = crate::db::queries::evaporate_pulse(&state.db, pid).await;
    }

    // 2. Delete the user row. CASCADE handles the rest.
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(me.id)
        .execute(&state.db)
        .await
        .map_err(AppError::Db)?;

    tracing::info!(user_id = %me.id, username = %me.username, "account deleted");

    Ok(Json(serde_json::json!({
        "deleted": true,
        "username": me.username,
    })))
}

/// GET /settings/blocks — list users I've blocked.
pub async fn list_blocks(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
) -> AppResult<Json<Vec<BlockOut>>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let rows: Vec<(uuid::Uuid, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT b.blocked_id, u.username, b.created_at
        FROM blocks b JOIN users u ON u.id = b.blocked_id
        WHERE b.blocker_id = $1
        ORDER BY b.created_at DESC
        "#,
    )
    .bind(me.id)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    let out: Vec<BlockOut> = rows
        .into_iter()
        .map(|(id, username, created_at)| BlockOut {
            user_id: id,
            username,
            created_at,
        })
        .collect();
    Ok(Json(out))
}

#[derive(Debug, Deserialize)]
pub struct BlockRequest {
    pub user_id: uuid::Uuid,
}

#[derive(Debug, Serialize)]
pub struct BlockOut {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// POST /settings/blocks — block a user.
pub async fn block_user(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Json(req): Json<BlockRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;
    if me.id == req.user_id {
        return Err(AppError::BadRequest("cannot block yourself".into()));
    }
    sqlx::query(
        "INSERT INTO blocks (blocker_id, blocked_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(me.id)
    .bind(req.user_id)
    .execute(&state.db)
    .await
    .map_err(AppError::Db)?;

    // Also remove the connection (both directions).
    sqlx::query("DELETE FROM connections WHERE (source_id = $1 AND target_id = $2) OR (source_id = $2 AND target_id = $1)")
        .bind(me.id)
        .bind(req.user_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Db)?;

    Ok(Json(serde_json::json!({ "blocked": req.user_id })))
}

/// DELETE /settings/blocks/:user_id — unblock a user.
pub async fn unblock_user(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    axum::extract::Path(target): axum::extract::Path<uuid::Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;
    sqlx::query("DELETE FROM blocks WHERE blocker_id = $1 AND blocked_id = $2")
        .bind(me.id)
        .bind(target)
        .execute(&state.db)
        .await
        .map_err(AppError::Db)?;
    Ok(Json(serde_json::json!({ "unblocked": target })))
}

/// GET /settings/saved — list my saved pulses.
pub async fn list_saved(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
) -> AppResult<Json<Vec<SavedPulseOut>>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let rows: Vec<(uuid::Uuid, uuid::Uuid, Vec<u8>, chrono::DateTime<chrono::Utc>, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT p.id, p.author_id, p.ciphertext, p.created_at, sp.personal_note, sp.saved_at
        FROM saved_pulses sp
        JOIN pulses p ON p.id = sp.pulse_id
        WHERE sp.user_id = $1 AND p.lifecycle <> 'evaporated'
        ORDER BY sp.saved_at DESC
        LIMIT 100
        "#,
    )
    .bind(me.id)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    let out: Vec<SavedPulseOut> = rows
        .into_iter()
        .map(|(id, author_id, ciphertext, created_at, note, saved_at)| SavedPulseOut {
            pulse_id: id,
            author_id,
            ciphertext_b64: base64::encode(&ciphertext),
            created_at,
            personal_note: note,
            saved_at,
        })
        .collect();
    Ok(Json(out))
}

#[derive(Debug, Serialize)]
pub struct SavedPulseOut {
    pub pulse_id: uuid::Uuid,
    pub author_id: uuid::Uuid,
    pub ciphertext_b64: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub personal_note: String,
    pub saved_at: chrono::DateTime<chrono::Utc>,
}

/// POST /pulses/:id/save-bookmark — add to saved_pulses with optional note.
#[derive(Debug, Deserialize)]
pub struct SaveBookmarkRequest {
    pub personal_note: Option<String>,
}

pub async fn save_bookmark(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    axum::extract::Path(pulse_id): axum::extract::Path<uuid::Uuid>,
    Json(req): Json<SaveBookmarkRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;
    let note = req.personal_note.unwrap_or_default();
    if note.len() > 200 {
        return Err(AppError::BadRequest("personal_note max 200 chars".into()));
    }
    sqlx::query(
        r#"
        INSERT INTO saved_pulses (user_id, pulse_id, personal_note)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id, pulse_id) DO UPDATE SET personal_note = $3
        "#,
    )
    .bind(me.id)
    .bind(pulse_id)
    .bind(&note)
    .execute(&state.db)
    .await
    .map_err(AppError::Db)?;

    Ok(Json(serde_json::json!({ "saved": pulse_id })))
}

/// DELETE /pulses/:id/save-bookmark — remove from saved.
pub async fn remove_bookmark(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    axum::extract::Path(pulse_id): axum::extract::Path<uuid::Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;
    sqlx::query("DELETE FROM saved_pulses WHERE user_id = $1 AND pulse_id = $2")
        .bind(me.id)
        .bind(pulse_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Db)?;
    Ok(Json(serde_json::json!({ "removed": pulse_id })))
}
