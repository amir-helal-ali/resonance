// ============================================================================
// resonance-backend/src/handlers/dms.rs
// End-to-end encrypted Direct Messages.
//
// The server stores ONLY:
//   - ciphertext (AES-GCM of the message body)
//   - ephemeral_pubkey (X25519 public key for this message)
//   - sender_id, recipient_id, timestamps
//
// The message body is decrypted client-side via X25519 ECDH between the
// sender's ephemeral key and the recipient's static Ed25519 key (converted
// to X25519). The server CANNOT decrypt messages.
//
// DMs auto-expire after 30 days (cron job 8 in cron/mod.rs prunes them).
// ============================================================================

use axum::{
    extract::{Path, Query, State},
    Json,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult},
    middleware::signature::VerifiedPubKey,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct SendDmRequest {
    pub recipient_id: Uuid,
    /// AES-GCM ciphertext of the message body, base64-encoded.
    pub ciphertext_b64: String,
    /// Ephemeral X25519 public key (32 bytes) for this message, base64.
    pub ephemeral_pubkey_b64: String,
}

#[derive(Debug, Serialize)]
pub struct SendDmResponse {
    pub message_id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct DmOut {
    pub id: Uuid,
    pub sender_id: Uuid,
    pub ciphertext_b64: String,
    pub ephemeral_pubkey_b64: String,
    pub read_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListDmsQuery {
    pub with_user: Uuid,
    pub limit: Option<i64>,
    pub before: Option<chrono::DateTime<chrono::Utc>>,
}

/// POST /dms — send an encrypted DM.
pub async fn send_dm(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Json(req): Json<SendDmRequest>,
) -> AppResult<Json<SendDmResponse>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    if me.id == req.recipient_id {
        return Err(AppError::BadRequest("cannot DM yourself".into()));
    }

    // 1. Check block: if recipient blocked me, reject silently (return 200 to
    //    avoid leaking the block state).
    let blocked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM blocks WHERE blocker_id = $1 AND blocked_id = $2",
    )
    .bind(req.recipient_id)
    .bind(me.id)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Db)?;
    if blocked > 0 {
        // Pretend success to avoid leaking block state.
        return Ok(Json(SendDmResponse {
            message_id: Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::days(30),
        }));
    }

    // 2. Decode ciphertext + ephemeral pubkey.
    let ciphertext = base64::decode(req.ciphertext_b64.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("ciphertext not base64: {e}")))?;
    let ephemeral_pubkey = base64::decode(req.ephemeral_pubkey_b64.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("ephemeral_pubkey not base64: {e}")))?;
    if ephemeral_pubkey.len() != 32 {
        return Err(AppError::BadRequest("ephemeral_pubkey must be 32 bytes".into()));
    }

    // 3. Canonical ordering: user_a < user_b.
    let (user_a, user_b) = if me.id < req.recipient_id {
        (me.id, req.recipient_id)
    } else {
        (req.recipient_id, me.id)
    };

    // 4. Insert.
    let row: (Uuid, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        r#"
        INSERT INTO direct_messages (user_a, user_b, sender_id, ciphertext, ephemeral_pubkey)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, created_at, expires_at
        "#,
    )
    .bind(user_a)
    .bind(user_b)
    .bind(me.id)
    .bind(&ciphertext)
    .bind(&ephemeral_pubkey)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Db)?;

    // 5. Notify the recipient in real-time via their personal WS channel.
    let mut r = state.redis.clone();
    let event = serde_json::json!({
        "type": "dm:new",
        "message_id": row.0,
        "from_user_id": me.id,
        "from_username": me.username,
        "created_at": row.1,
    });
    let _: () = r
        .publish(format!("user:{}", req.recipient_id), event.to_string())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("publish dm:new: {e}")))?;

    // 6. Insert a notification row (so the recipient sees it in /notifications).
    sqlx::query(
        r#"
        INSERT INTO notifications (user_id, kind, actor_id, target_type, target_id, payload)
        VALUES ($1, 'comment', $2, 'dm', $3, $4)
        "#,
    )
    .bind(req.recipient_id)
    .bind(me.id)
    .bind(row.0)
    .bind(serde_json::json!({ "from_username": me.username }))
    .execute(&state.db)
    .await
    .map_err(AppError::Db)?;

    Ok(Json(SendDmResponse {
        message_id: row.0,
        created_at: row.1,
        expires_at: row.2,
    }))
}

/// GET /dms?with_user=...&limit=...&before=... — list DMs with a specific user.
pub async fn list_dms(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Query(q): Query<ListDmsQuery>,
) -> AppResult<Json<Vec<DmOut>>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let (user_a, user_b) = if me.id < q.with_user {
        (me.id, q.with_user)
    } else {
        (q.with_user, me.id)
    };

    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let rows: Vec<(Uuid, Uuid, Vec<u8>, Vec<u8>, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT id, sender_id, ciphertext, ephemeral_pubkey, read_at, created_at, expires_at
        FROM direct_messages
        WHERE user_a = $1 AND user_b = $2
          AND ($3::timestamptz IS NULL OR created_at < $3)
          AND expires_at > now()
        ORDER BY created_at DESC
        LIMIT $4
        "#,
    )
    .bind(user_a)
    .bind(user_b)
    .bind(q.before)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    let out: Vec<DmOut> = rows
        .into_iter()
        .map(|r| DmOut {
            id: r.0,
            sender_id: r.1,
            ciphertext_b64: base64::encode(&r.2),
            ephemeral_pubkey_b64: base64::encode(&r.3),
            read_at: r.4,
            created_at: r.5,
            expires_at: r.6,
        })
        .collect();

    // Mark messages from the other user as read.
    sqlx::query(
        r#"
        UPDATE direct_messages
        SET read_at = now()
        WHERE user_a = $1 AND user_b = $2 AND sender_id = $2 AND read_at IS NULL
        "#,
    )
    .bind(user_a)
    .bind(user_b)
    .execute(&state.db)
    .await
    .map_err(AppError::Db)?;

    Ok(Json(out))
}

/// GET /dms/conversations — list my DM conversations (latest message per partner).
pub async fn list_conversations(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
) -> AppResult<Json<Vec<ConversationOut>>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let rows: Vec<(Uuid, Uuid, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT DISTINCT ON (partner_id)
            partner_id,
            sender_id,
            created_at
        FROM (
            SELECT user_b AS partner_id, sender_id, created_at
            FROM direct_messages WHERE user_a = $1 AND expires_at > now()
            UNION ALL
            SELECT user_a AS partner_id, sender_id, created_at
            FROM direct_messages WHERE user_b = $1 AND expires_at > now()
        ) AS conv
        ORDER BY partner_id, created_at DESC
        "#,
    )
    .bind(me.id)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    // Fetch unread counts per partner.
    let mut out = Vec::with_capacity(rows.len());
    for (partner_id, _last_sender, last_at) in rows {
        let (user_a, user_b) = if me.id < partner_id {
            (me.id, partner_id)
        } else {
            (partner_id, me.id)
        };
        let unread: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM direct_messages
            WHERE user_a = $1 AND user_b = $2 AND sender_id = $2 AND read_at IS NULL
            "#,
        )
        .bind(user_a)
        .bind(user_b)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Db)?;

        // Fetch partner username.
        let username: Option<(String,)> = sqlx::query_as("SELECT username FROM users WHERE id = $1")
            .bind(partner_id)
            .fetch_optional(&state.db)
            .await
            .map_err(AppError::Db)?;

        out.push(ConversationOut {
            partner_id,
            partner_username: username.map(|(u,)| u).unwrap_or_default(),
            unread_count: unread as u32,
            last_message_at: last_at,
        });
    }
    // Sort by last message time desc.
    out.sort_by(|a, b| b.last_message_at.cmp(&a.last_message_at));
    Ok(Json(out))
}

#[derive(Debug, Serialize)]
pub struct ConversationOut {
    pub partner_id: Uuid,
    pub partner_username: String,
    pub unread_count: u32,
    pub last_message_at: chrono::DateTime<chrono::Utc>,
}
