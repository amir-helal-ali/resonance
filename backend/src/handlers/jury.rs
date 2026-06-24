// ============================================================================
// resonance-backend/src/handlers/jury.rs
// The Transient Jury — a randomly-selected panel of 5 users who vote on
// flagged content. Summoned when:
//   - AI moderation score ≥ 0.9, OR
//   - A pulse receives ≥3 user reports.
//
// Quorum = 3 votes. Verdict = majority. The jury panel auto-expires after
// 24h (verdict 'expire' if no quorum).
// ============================================================================

use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult},
    middleware::signature::VerifiedPubKey,
    state::AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};

#[derive(Debug, Serialize)]
pub struct JuryOut {
    pub id: Uuid,
    pub pulse_id: Uuid,
    pub juror_ids: Vec<Uuid>,
    pub final_verdict: String,
    pub summoned_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub concluded_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CastVoteRequest {
    pub vote: String, // "uphold" | "release"
}

#[derive(Debug, Serialize)]
pub struct CastVoteResponse {
    pub final_verdict: String,
    pub votes_for_release: i32,
    pub votes_for_uphold: i32,
}

/// Summon a jury for a pulse. Returns the jury panel id (or None if a panel
/// already exists). Called from the moderation cron or the report handler.
pub async fn summon_for_pulse(pool: &PgPool, pulse_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    // 1. Check if a panel already exists for this pulse.
    let existing: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM jury_panels WHERE pulse_id = $1")
            .bind(pulse_id)
            .fetch_optional(pool)
            .await?;
    if let Some((id,)) = existing {
        return Ok(Some(id));
    }

    // 2. Randomly select 5 jurors. We use `ORDER BY random()` — for large
    //    user tables you'd use TABLESAMPLE instead for performance.
    let jurors: Vec<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id FROM users
        WHERE is_quarantined = false AND email_verified = true
        ORDER BY random()
        LIMIT 5
        "#,
    )
    .fetch_all(pool)
    .await?;
    if jurors.len() < 5 {
        tracing::warn!(pulse_id = %pulse_id, "not enough users to summon a jury");
        return Ok(None);
    }
    let juror_ids: Vec<Uuid> = jurors.into_iter().map(|(u,)| u).collect();

    // 3. Insert the panel.
    let row: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO jury_panels (pulse_id, juror_ids)
        VALUES ($1, $2)
        RETURNING id
        "#,
    )
    .bind(pulse_id)
    .bind(&juror_ids)
    .fetch_one(pool)
    .await?;

    // 4. Notify each juror via their personal Redis channel.
    // (In production we'd also send a push notification.)
    tracing::info!(panel_id = %row.0, pulse_id = %pulse_id, "jury summoned");
    Ok(Some(row.0))
}

/// GET /jury/summoned — list panels where the requester is a juror.
pub async fn list_summoned(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
) -> AppResult<Json<Vec<JuryOut>>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    // The `juror_ids` column is UUID[]; we use the `@>` (contains) operator.
    let rows: Vec<(Uuid, Uuid, Vec<Uuid>, String, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
        r#"
        SELECT id, pulse_id, juror_ids, final_verdict::text, summoned_at, expires_at, concluded_at
        FROM jury_panels
        WHERE $1 = ANY(juror_ids) AND final_verdict = 'pending'
        ORDER BY expires_at ASC
        "#,
    )
    .bind(me.id)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    let out: Vec<JuryOut> = rows
        .into_iter()
        .map(|r| JuryOut {
            id: r.0,
            pulse_id: r.1,
            juror_ids: r.2,
            final_verdict: r.3,
            summoned_at: r.4,
            expires_at: r.5,
            concluded_at: r.6,
        })
        .collect();
    Ok(Json(out))
}

/// POST /jury/:panel_id/vote — cast a vote. Once quorum (3) is reached,
/// the final verdict is set and the pulse is either released or quarantined.
pub async fn cast_vote(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Path(panel_id): Path<Uuid>,
    Json(req): Json<CastVoteRequest>,
) -> AppResult<Json<CastVoteResponse>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let vote = match req.vote.as_str() {
        "uphold" | "release" => req.vote.clone(),
        _ => return Err(AppError::BadRequest("vote must be 'uphold' or 'release'".into())),
    };

    // 1. Load the panel.
    let panel: (Uuid, Vec<Uuid>, sqlx::types::Json<serde_json::Value>, String, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        r#"
        SELECT pulse_id, juror_ids, votes, final_verdict::text, expires_at
        FROM jury_panels WHERE id = $1 FOR UPDATE
        "#,
    )
    .bind(panel_id)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Db)?;

    // 2. Validate the requester is a juror and the panel is still pending.
    if !panel.1.contains(&me.id) {
        return Err(AppError::Unauthorized);
    }
    if panel.3 != "pending" {
        return Err(AppError::BadRequest("panel already concluded".into()));
    }
    if chrono::Utc::now() > panel.4 {
        return Err(AppError::BadRequest("panel expired".into()));
    }

    // 3. Update the votes JSON. We use the juror's UUID as the key.
    let mut votes_obj = panel
        .2
        .as_object()
        .cloned()
        .unwrap_or_default();
    votes_obj.insert(me.id.to_string(), serde_json::Value::String(vote.clone()));
    let votes_json = serde_json::Value::Object(votes_obj);

    // 4. Count votes.
    let for_release = votes_json
        .as_object()
        .map(|o| o.values().filter(|v| v.as_str() == Some("release")).count())
        .unwrap_or(0) as i32;
    let for_uphold = votes_json
        .as_object()
        .map(|o| o.values().filter(|v| v.as_str() == Some("uphold")).count())
        .unwrap_or(0) as i32;

    // 5. Check quorum.
    let total = for_release + for_uphold;
    let (final_verdict, concluded_at) = if total >= 3 {
        let v = if for_release > for_uphold { "release" } else { "uphold" };
        (v.to_string(), Some(chrono::Utc::now()))
    } else {
        ("pending".to_string(), None)
    };

    // 6. Apply the verdict.
    sqlx::query(
        r#"
        UPDATE jury_panels
        SET votes = $1, final_verdict = $2::jury_verdict, concluded_at = $3
        WHERE id = $4
        "#,
    )
    .bind(&votes_json)
    .bind(&final_verdict)
    .bind(concluded_at)
    .bind(panel_id)
    .execute(&state.db)
    .await
    .map_err(AppError::Db)?;

    // 7. Side-effects: if the verdict is 'uphold', quarantine the pulse.
    if final_verdict == "uphold" {
        sqlx::query("UPDATE pulses SET is_preserved = false WHERE id = $1")
            .bind(panel.0)
            .execute(&state.db)
            .await
            .map_err(AppError::Db)?;
        // Evaporate immediately.
        crate::db::queries::evaporate_pulse(&state.db, panel.0)
            .await
            .map_err(AppError::Db)?;
    } else if final_verdict == "release" {
        sqlx::query("UPDATE moderation_queue SET verdict = 'released' WHERE pulse_id = $1")
            .bind(panel.0)
            .execute(&state.db)
            .await
            .map_err(AppError::Db)?;
    }

    // 8. Notify all jurors that the panel concluded.
    let mut r = state.redis.clone();
    let event = serde_json::json!({
        "type": "jury:concluded",
        "panel_id": panel_id,
        "final_verdict": final_verdict,
    });
    for juror in &panel.1 {
        let _: () = r
            .publish(format!("user:{}", juror), event.to_string())
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("publish jury: {e}")))?;
    }

    Ok(Json(CastVoteResponse {
        final_verdict,
        votes_for_release: for_release,
        votes_for_uphold: for_uphold,
    }))
}
