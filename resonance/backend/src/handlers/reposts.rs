// ============================================================================
// resonance-backend/src/handlers/reposts.rs
// Repost + quote-repost mechanics.
//
// A repost is a NEW pulse that references an existing pulse. The new pulse's
// ciphertext is the (optional) quote comment. The original author gets a
// notification + a resonance bump.
// ============================================================================

use axum::{
    extract::{Path, State},
    Json,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::queries,
    errors::{AppError, AppResult},
    middleware::signature::VerifiedPubKey,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct RepostRequest {
    pub original_pulse_id: Uuid,
    /// Quote comment ciphertext (base64). If empty, it's a plain repost.
    pub quote_ciphertext_b64: Option<String>,
    /// Wrapped per-pulse key for the quote (base64). Required if quote present.
    pub quote_wrapped_key_b64: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RepostResponse {
    pub repost_pulse_id: Uuid,
    pub is_quote: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// POST /pulses/repost — create a repost (with optional quote).
pub async fn repost(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Json(req): Json<RepostRequest>,
) -> AppResult<Json<RepostResponse>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    // 1. Fetch the original pulse.
    let original: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, author_id FROM pulses WHERE id = $1 AND lifecycle <> 'evaporated'",
    )
    .bind(req.original_pulse_id)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let (orig_id, orig_author_id) = original.ok_or(AppError::NotFound)?;
    if orig_author_id == me.id {
        return Err(AppError::BadRequest("cannot repost your own pulse".into()));
    }

    // 2. Check block.
    let blocked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM blocks WHERE blocker_id = $1 AND blocked_id = $2",
    )
    .bind(orig_author_id)
    .bind(me.id)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Db)?;
    if blocked > 0 {
        return Err(AppError::Unauthorized);
    }

    let is_quote = req.quote_ciphertext_b64.is_some() && req.quote_wrapped_key_b64.is_some();

    // 3. Create the new pulse (the repost body).
    let ciphertext = if is_quote {
        base64::decode(req.quote_ciphertext_b64.as_ref().unwrap().as_bytes())
            .map_err(|e| AppError::BadRequest(format!("quote ciphertext not base64: {e}")))?
    } else {
        // Plain repost: empty ciphertext (just a reference).
        Vec::new()
    };
    let wrapped_key = if is_quote {
        base64::decode(req.quote_wrapped_key_b64.as_ref().unwrap().as_bytes())
            .map_err(|e| AppError::BadRequest(format!("quote wrapped_key not base64: {e}")))?
    } else {
        Vec::new()
    };

    // 4. Insert encryption key.
    let key_row: (Uuid,) = sqlx::query_as(
        "INSERT INTO encryption_keys (wrapped_key) VALUES ($1) RETURNING id",
    )
    .bind(&wrapped_key)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Db)?;
    let encryption_key_id = key_row.0;

    // 5. Insert the repost pulse.
    let pulse = queries::insert_pulse(&state.db, me.id, &ciphertext, encryption_key_id)
        .await
        .map_err(AppError::Db)?;

    // 6. Insert the repost relationship.
    sqlx::query(
        r#"
        INSERT INTO reposts (repost_pulse_id, original_pulse_id, is_quote)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(pulse.id)
    .bind(orig_id)
    .bind(is_quote)
    .execute(&state.db)
    .await
    .map_err(AppError::Db)?;

    // 7. Bump resonance me → orig_author.
    let _ = queries::upsert_connection(&state.db, me.id, orig_author_id, 6)
        .await
        .map_err(AppError::Db)?;

    // 8. Notify the original author.
    sqlx::query(
        r#"
        INSERT INTO notifications (user_id, kind, actor_id, target_type, target_id, payload)
        VALUES ($1, 'echo', $2, 'pulse', $3, $4)
        "#,
    )
    .bind(orig_author_id)
    .bind(me.id)
    .bind(pulse.id)
    .bind(serde_json::json!({
        "repost": true,
        "is_quote": is_quote,
        "original_pulse_id": orig_id,
    }))
    .execute(&state.db)
    .await
    .map_err(AppError::Db)?;

    // 9. Real-time push to original author.
    let mut r = state.redis.clone();
    let event = serde_json::json!({
        "type": "pulse:repost",
        "repost_pulse_id": pulse.id,
        "original_pulse_id": orig_id,
        "actor_id": me.id,
        "actor_username": me.username,
        "is_quote": is_quote,
    });
    let _: () = r
        .publish(format!("user:{}", orig_author_id), event.to_string())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("publish repost: {e}")))?;

    // 10. Add to glow feed.
    let now_ms = chrono::Utc::now().timestamp_millis() as f64;
    let _: () = r
        .zadd("feed:glow", pulse.id.to_string(), now_ms)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("zadd feed:glow: {e}")))?;
    let event = serde_json::json!({
        "type": "pulse:new",
        "pulse_id": pulse.id,
        "author": me.username,
        "created_at": pulse.created_at,
        "is_repost": true,
        "original_pulse_id": orig_id,
    });
    let _: () = r
        .publish("feed:new", event.to_string())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("publish feed:new: {e}")))?;

    state.metrics.pulses_created.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    Ok(Json(RepostResponse {
        repost_pulse_id: pulse.id,
        is_quote,
        created_at: pulse.created_at,
    }))
}

/// GET /pulses/:id/reposts — list reposts of a pulse.
pub async fn list_reposts(
    State(state): State<AppState>,
    VerifiedPubKey(_pubkey): VerifiedPubKey,
    Path(pulse_id): Path<Uuid>,
) -> AppResult<Json<Vec<RepostOut>>> {
    let rows: Vec<(Uuid, Uuid, bool, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT r.repost_pulse_id, p.author_id, r.is_quote, r.created_at
        FROM reposts r
        JOIN pulses p ON p.id = r.repost_pulse_id
        WHERE r.original_pulse_id = $1
        ORDER BY r.created_at DESC
        LIMIT 50
        "#,
    )
    .bind(pulse_id)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    let out: Vec<RepostOut> = rows
        .into_iter()
        .map(|(rp_id, author_id, is_q, ts)| RepostOut {
            repost_pulse_id: rp_id,
            author_id,
            is_quote: is_q,
            created_at: ts,
        })
        .collect();
    Ok(Json(out))
}

#[derive(Debug, Serialize)]
pub struct RepostOut {
    pub repost_pulse_id: Uuid,
    pub author_id: Uuid,
    pub is_quote: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
