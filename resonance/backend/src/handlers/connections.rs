// ============================================================================
// resonance-backend/src/handlers/connections.rs
// The "صدى" (resonance) relationship. There is no follow; there is only
// sync. Each sync raises the resonance_score (capped at 100) and refreshes
// `last_interaction_at` so the 7-day decay clock resets.
//
// Co-Resonance friend suggestions use Jaccard similarity over the set of
// users each user resonates with above threshold.
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
pub struct SyncRequest {
    pub target_user_id: Uuid,
    /// How strongly to sync (1-10). Higher = bigger resonance bump.
    /// Default is 5 (a "neutral" sync). Echoing a pulse passes 7. Saving
    /// passes 9. A bare profile visit passes 1.
    pub strength: Option<i16>,
}

#[derive(Debug, Serialize)]
pub struct SyncResponse {
    pub new_score: i16,
    pub aura_now_visible: bool,
}

#[derive(Debug, Serialize)]
pub struct SuggestionEntry {
    pub user_id: Uuid,
    pub username: String,
    pub jaccard: f32,
}

/// POST /connections/sync — bump resonance with another user.
pub async fn sync(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Json(req): Json<SyncRequest>,
) -> AppResult<Json<SyncResponse>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    if me.id == req.target_user_id {
        return Err(AppError::BadRequest("cannot sync with yourself".into()));
    }

    let strength = req.strength.unwrap_or(5).clamp(1, 10);
    let delta = strength * 2; // 2..20 points per sync

    let prev_score = queries::get_resonance(&state.db, me.id, req.target_user_id)
        .await
        .map_err(AppError::Db)?;

    let conn = queries::upsert_connection(&state.db, me.id, req.target_user_id, delta)
        .await
        .map_err(AppError::Db)?;

    // If we just crossed the 50% threshold, broadcast a presence notification
    // to the target so their UI can render the "Pulsing Now" aura.
    let aura_now_visible = prev_score <= 50 && conn.resonance_score > 50;
    if aura_now_visible {
        let mut r = state.redis.clone();
        let event = serde_json::json!({
            "type": "resonance:threshold",
            "from_user_id": me.id,
            "from_username": me.username,
            "new_score": conn.resonance_score,
        });
        let _: () = r
            .publish(format!("user:{}", req.target_user_id), event.to_string())
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("publish resonance: {e}")))?;
    }

    Ok(Json(SyncResponse {
        new_score: conn.resonance_score,
        aura_now_visible,
    }))
}

/// GET /connections — list the requesting user's resonances (above threshold 5).
pub async fn list_my_connections(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
) -> AppResult<Json<Vec<ConnectionOut>>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let rows: Vec<(Uuid, i16, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT target_id, resonance_score, last_interaction_at
        FROM connections
        WHERE source_id = $1 AND resonance_score >= 5
        ORDER BY resonance_score DESC, last_interaction_at DESC
        LIMIT 200
        "#,
    )
    .bind(me.id)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    let out: Vec<ConnectionOut> = rows
        .into_iter()
        .map(|(target_id, score, ts)| ConnectionOut {
            target_user_id: target_id,
            resonance_score: score,
            last_interaction_at: ts,
        })
        .collect();
    Ok(Json(out))
}

#[derive(Debug, Serialize)]
pub struct ConnectionOut {
    pub target_user_id: Uuid,
    pub resonance_score: i16,
    pub last_interaction_at: chrono::DateTime<chrono::Utc>,
}

/// GET /connections/suggest — Co-Resonance friend suggestions (Jaccard).
///
/// For the requester, fetch the set of users they resonate with above 30.
/// For each of THOSE users, fetch who they resonate with above 30.
/// Compute Jaccard similarity between (me's set) and (each candidate's set).
/// Return the top 10 candidates the requester doesn't already sync with.
pub async fn suggest(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
) -> AppResult<Json<Vec<SuggestionEntry>>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    // 1. My set of resonances.
    let mine: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT target_id FROM connections
        WHERE source_id = $1 AND resonance_score >= 30
        "#,
    )
    .bind(me.id)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    if mine.is_empty() {
        return Ok(Json(vec![]));
    }
    let mine_set: std::collections::HashSet<Uuid> = mine.iter().copied().collect();

    // 2. For each user in `mine`, fetch their resonances.
    let mut candidate_counts: std::collections::HashMap<Uuid, std::collections::HashSet<Uuid>> =
        std::collections::HashMap::new();
    for u in &mine {
        let theirs: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT target_id FROM connections WHERE source_id = $1 AND resonance_score >= 30"#,
        )
        .bind(u)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Db)?;

        for t in theirs {
            if t == me.id || mine_set.contains(&t) {
                continue; // skip self and already-connected
            }
            candidate_counts
                .entry(t)
                .or_default()
                .insert(*u);
        }
    }

    // 3. Compute Jaccard: |intersection| / |union| where:
    //    intersection = candidate's supporters ∩ mine
    //    union = candidate's supporters ∪ mine
    //    (candidate's supporters = the set of users in `mine` who also resonate with them)
    let mut scored: Vec<(Uuid, f32)> = candidate_counts
        .into_iter()
        .map(|(cand, supporters)| {
            let inter = supporters.intersection(&mine_set).count() as f32;
            let union = supporters.union(&mine_set).count() as f32;
            let j = if union > 0.0 { inter / union } else { 0.0 };
            (cand, j)
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(10);

    // 4. Fetch usernames.
    let mut out = Vec::with_capacity(scored.len());
    for (uid, j) in scored {
        let row: Option<(String,)> = sqlx::query_as("SELECT username FROM users WHERE id = $1")
            .bind(uid)
            .fetch_optional(&state.db)
            .await
            .map_err(AppError::Db)?;
        if let Some((username,)) = row {
            out.push(SuggestionEntry {
                user_id: uid,
                username,
                jaccard: j,
            });
        }
    }
    Ok(Json(out))
}

/// DELETE /connections/:target — manually un-sync from a user.
pub async fn unsync(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Path(target): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    sqlx::query("DELETE FROM connections WHERE source_id = $1 AND target_id = $2")
        .bind(me.id)
        .bind(target)
        .execute(&state.db)
        .await
        .map_err(AppError::Db)?;

    Ok(Json(serde_json::json!({ "unsynced": target })))
}
