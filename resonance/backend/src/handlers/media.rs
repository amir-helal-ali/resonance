// ============================================================================
// resonance-backend/src/handlers/media.rs
// Encrypted media attachments.
//
// The server stores ONLY opaque AES-GCM ciphertext + metadata (mime_type,
// dimensions, duration, original_sha256 for dedup). Decryption happens
// client-side with the per-pulse key (same key used for the pulse body).
// ============================================================================

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult},
    middleware::signature::VerifiedPubKey,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct UploadMediaRequest {
    pub pulse_id: Uuid,
    pub kind: String,             // "image" | "video" | "audio"
    pub mime_type: String,
    pub ciphertext_b64: String,
    pub iv_b64: String,           // 12 bytes
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_ms: Option<i32>,
    pub original_sha256_b64: String, // 32 bytes
}

#[derive(Debug, Serialize)]
pub struct MediaOut {
    pub id: Uuid,
    pub pulse_id: Uuid,
    pub kind: String,
    pub mime_type: String,
    pub ciphertext_b64: String,
    pub iv_b64: String,
    pub width: i32,
    pub height: i32,
    pub duration_ms: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListMediaQuery {
    pub pulse_id: Uuid,
}

/// POST /media — upload an encrypted media attachment.
pub async fn upload_media(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Json(req): Json<UploadMediaRequest>,
) -> AppResult<Json<MediaOut>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    // 1. Verify ownership: the pulse must be authored by the requester.
    let pulse: Option<(Uuid,)> = sqlx::query_as("SELECT author_id FROM pulses WHERE id = $1")
        .bind(req.pulse_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Db)?;
    let (author_id,) = pulse.ok_or(AppError::NotFound)?;
    if author_id != me.id {
        return Err(AppError::Unauthorized);
    }

    // 2. Validate kind.
    if !["image", "video", "audio"].contains(&req.kind.as_str()) {
        return Err(AppError::BadRequest("kind must be image|video|audio".into()));
    }

    // 3. Decode base64.
    let ciphertext = base64::decode(req.ciphertext_b64.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("ciphertext not base64: {e}")))?;
    let iv = base64::decode(req.iv_b64.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("iv not base64: {e}")))?;
    if iv.len() != 12 {
        return Err(AppError::BadRequest("iv must be 12 bytes".into()));
    }
    let original_sha256 = base64::decode(req.original_sha256_b64.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("original_sha256 not base64: {e}")))?;
    if original_sha256.len() != 32 {
        return Err(AppError::BadRequest("original_sha256 must be 32 bytes".into()));
    }

    // 4. Limit size (10 MB ciphertext).
    if ciphertext.len() > 10 * 1024 * 1024 {
        return Err(AppError::BadRequest("media exceeds 10 MB limit".into()));
    }

    // 5. Insert.
    let row: (Uuid, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        r#"
        INSERT INTO media_attachments
            (pulse_id, kind, mime_type, ciphertext, iv, width, height, duration_ms, original_sha256)
        VALUES ($1, $2::media_kind, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id, created_at
        "#,
    )
    .bind(req.pulse_id)
    .bind(&req.kind)
    .bind(&req.mime_type)
    .bind(&ciphertext)
    .bind(&iv)
    .bind(req.width.unwrap_or(0))
    .bind(req.height.unwrap_or(0))
    .bind(req.duration_ms.unwrap_or(0))
    .bind(&original_sha256)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Db)?;

    Ok(Json(MediaOut {
        id: row.0,
        pulse_id: req.pulse_id,
        kind: req.kind,
        mime_type: req.mime_type,
        ciphertext_b64: req.ciphertext_b64,
        iv_b64: req.iv_b64,
        width: req.width.unwrap_or(0),
        height: req.height.unwrap_or(0),
        duration_ms: req.duration_ms.unwrap_or(0),
        created_at: row.1,
    }))
}

/// GET /media?pulse_id=... — list media for a pulse.
pub async fn list_media(
    State(state): State<AppState>,
    VerifiedPubKey(_pubkey): VerifiedPubKey,
    Query(q): Query<ListMediaQuery>,
) -> AppResult<Json<Vec<MediaOut>>> {
    let rows: Vec<(Uuid, Uuid, String, String, Vec<u8>, Vec<u8>, i32, i32, i32, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT id, pulse_id, kind::text, mime_type, ciphertext, iv, width, height, duration_ms, created_at
        FROM media_attachments
        WHERE pulse_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(q.pulse_id)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    let out: Vec<MediaOut> = rows
        .into_iter()
        .map(|r| MediaOut {
            id: r.0,
            pulse_id: r.1,
            kind: r.2,
            mime_type: r.3,
            ciphertext_b64: base64::encode(&r.4),
            iv_b64: base64::encode(&r.5),
            width: r.6,
            height: r.7,
            duration_ms: r.8,
            created_at: r.9,
        })
        .collect();
    Ok(Json(out))
}
