// ============================================================================
// resonance-backend/src/handlers/interactions.rs
// Echo, save, comment on a pulse. Each interaction:
//   1. Inserts a row in `interactions`.
//   2. Bumps `pulses.last_interaction_at` (delays Immortal Decay).
//   3. Bumps `connections.resonance_score` between actor and pulse author.
//   4. Publishes a real-time event to the pulse author's WS channel.
// ============================================================================

use axum::{
    extract::{Path, State},
    Json,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use uuid::Uuid;

use crate::{
    db::queries,
    errors::{AppError, AppResult},
    middleware::signature::VerifiedPubKey,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct EchoRequest {}

#[derive(Debug, Deserialize)]
pub struct SaveRequest {}

#[derive(Debug, Deserialize)]
pub struct CommentRequest {
    /// Encrypted comment body (base64). The server is structurally blind.
    pub ciphertext_b64: String,
}

#[derive(Debug, Serialize)]
pub struct InteractionResponse {
    pub interaction_id: Uuid,
    pub new_resonance_score: i16,
}

/// POST /pulses/:id/echo — amplify the pulse.
pub async fn echo(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Path(pulse_id): Path<Uuid>,
) -> AppResult<Json<InteractionResponse>> {
    record_interaction(&state, pubkey, pulse_id, "echo", None, 7).await
}

/// POST /pulses/:id/save — bookmark the pulse.
pub async fn save(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Path(pulse_id): Path<Uuid>,
) -> AppResult<Json<InteractionResponse>> {
    record_interaction(&state, pubkey, pulse_id, "save", None, 9).await
}

/// POST /pulses/:id/comment — add an encrypted comment.
pub async fn comment(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Path(pulse_id): Path<Uuid>,
    Json(req): Json<CommentRequest>,
) -> AppResult<Json<InteractionResponse>> {
    let payload = base64::decode(req.ciphertext_b64.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("ciphertext not base64: {e}")))?;
    record_interaction(&state, pubkey, pulse_id, "comment", Some(&payload), 8).await
}

/// POST /pulses/:id/report — flag the pulse for jury review.
#[derive(Debug, Deserialize)]
pub struct ReportRequest {
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct ReportResponse {
    pub report_id: Uuid,
    pub jury_summoned: bool,
}

pub async fn report(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Path(pulse_id): Path<Uuid>,
    Json(req): Json<ReportRequest>,
) -> AppResult<Json<ReportResponse>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    if req.reason.len() < 1 || req.reason.len() > 500 {
        return Err(AppError::BadRequest("reason length 1..500".into()));
    }

    let row: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO pulse_reports (pulse_id, reporter_id, reason)
        VALUES ($1, $2, $3)
        ON CONFLICT (pulse_id, reporter_id) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(pulse_id)
    .bind(me.id)
    .bind(&req.reason)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Db)?;
    let report_id = row.0;

    // Count reports on this pulse. If ≥3, summon a jury.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pulse_reports WHERE pulse_id = $1")
        .bind(pulse_id)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Db)?;

    let jury_summoned = if count >= 3 {
        crate::handlers::jury::summon_for_pulse(&state.db, pulse_id)
            .await
            .map_err(AppError::Db)?
            .is_some()
    } else {
        false
    };

    Ok(Json(ReportResponse { report_id, jury_summoned }))
}

// ----------------------------------------------------------------------------
// Internal helper — record an interaction and bump resonance.
// ----------------------------------------------------------------------------
async fn record_interaction(
    state: &AppState,
    pubkey: [u8; 32],
    pulse_id: Uuid,
    kind: &str,
    payload: Option<&[u8]>,
    resonance_bump: i16,
) -> AppResult<Json<InteractionResponse>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    // 1. Fetch the pulse to know its author.
    let pulse: Option<(Uuid,)> = sqlx::query_as("SELECT author_id FROM pulses WHERE id = $1")
        .bind(pulse_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Db)?;
    let author_id = pulse.ok_or(AppError::NotFound)?.0;
    if author_id == me.id {
        return Err(AppError::BadRequest("cannot interact with own pulse".into()));
    }

    // 2. Insert the interaction row. UNIQUE constraint dedupes (pulse, actor, kind).
    let row: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO interactions (pulse_id, actor_id, kind, payload)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (pulse_id, actor_id, kind) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(pulse_id)
    .bind(me.id)
    .bind(kind)
    .bind(payload)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let interaction_id = match row {
        Some((id,)) => id,
        None => {
            // Already interacted. Idempotent — return current score.
            let score = queries::get_resonance(&state.db, me.id, author_id)
                .await
                .map_err(AppError::Db)?;
            return Ok(Json(InteractionResponse {
                interaction_id: Uuid::nil(),
                new_resonance_score: score,
            }));
        }
    };

    // 3. Bump the pulse's last_interaction_at.
    queries::touch_pulse_interaction(&state.db, pulse_id)
        .await
        .map_err(AppError::Db)?;

    // 4. Bump resonance actor → author.
    let conn = queries::upsert_connection(&state.db, me.id, author_id, resonance_bump)
        .await
        .map_err(AppError::Db)?;

    // 5. Publish a real-time event to the author.
    let mut r = state.redis.clone();
    let event = serde_json::json!({
        "type": "pulse:interaction",
        "pulse_id": pulse_id,
        "actor_id": me.id,
        "actor_username": me.username,
        "kind": kind,
        "new_resonance_score": conn.resonance_score,
    });
    let _: () = r
        .publish(format!("user:{}", author_id), event.to_string())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("publish interaction: {e}")))?;

    // 6. Bump the metrics counter for this interaction kind.
    match kind {
        "echo"    => { state.metrics.echoes.fetch_add(1, Ordering::Relaxed); }
        "save"    => { state.metrics.saves.fetch_add(1, Ordering::Relaxed); }
        "comment" => { state.metrics.comments.fetch_add(1, Ordering::Relaxed); }
        _ => {}
    }

    Ok(Json(InteractionResponse {
        interaction_id,
        new_resonance_score: conn.resonance_score,
    }))
}
