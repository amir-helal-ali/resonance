// ============================================================================
// resonance-backend/src/presence/mod.rs
// The "Pulsing Now" aura + Passing Traces manager.
//
// Two responsibilities:
//   1. PULSING NOW  — when user A visits user B's profile, if the resonance
//      score (A→B or B→A) is > 50%, A's id is added to a Redis SET with a
//      60s TTL. The aura only renders for pairs above this threshold.
//      Strangers never see presence — a core privacy promise.
//   2. PASSING TRACES — every profile visit is recorded in the `traces`
//      table with a 7-day TTL. The visitor can choose to be anonymous
//      (the visited sees "someone visited") or named.
// ============================================================================

use axum::{extract::State, Json};
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
pub struct PulsePresenceRequest {
    pub target_user_id: Uuid,
    /// Whether the visitor wants to leave a named trace.
    /// false = anonymous (server-stripped visitor_id in the trace row).
    pub leave_named_trace: bool,
}

#[derive(Debug, Serialize)]
pub struct PulsePresenceResponse {
    /// Whether the golden aura is now visible to the target.
    pub aura_visible: bool,
    /// How many other users are currently "Pulsing Now" on this target.
    pub pulsing_now_count: u32,
}

#[derive(Debug, Serialize)]
pub struct PresenceEntry {
    pub user_id: Uuid,
    pub username: String,
}

/// POST /presence/pulse — record a profile visit, possibly set the aura.
pub async fn pulse_presence(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Json(req): Json<PulsePresenceRequest>,
) -> AppResult<Json<PulsePresenceResponse>> {
    // 1. Resolve the visitor.
    let visitor = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    if visitor.id == req.target_user_id {
        return Err(AppError::BadRequest("cannot pulse yourself".into()));
    }

    // 2. Compute resonance in BOTH directions (the relationship is asymmetric).
    let ab = queries::get_resonance(&state.db, visitor.id, req.target_user_id)
        .await
        .map_err(AppError::Db)?;
    let ba = queries::get_resonance(&state.db, req.target_user_id, visitor.id)
        .await
        .map_err(AppError::Db)?;
    let best = ab.max(ba);

    // 3. Insert the trace. The kind is `named` only if the visitor opted in
    //    AND the resonance is high enough that naming makes sense.
    let kind = if req.leave_named_trace && best >= 50 {
        "named"
    } else {
        "anonymous"
    };
    let trace = match kind {
        "named" => queries::insert_trace(&state.db, req.target_user_id, Some(visitor.id), "named"),
        _ => queries::insert_trace(&state.db, req.target_user_id, None, "anonymous"),
    }
    .await
    .map_err(AppError::Db)?;
    tracing::info!(trace_id = %trace.id, kind, "trace recorded");

    // 4. Aura logic: only set Pulsing Now if resonance > 50%.
    let mut conn = state.redis.clone();
    let aura_visible = best > 50;
    if aura_visible {
        let key = format!("presence:{}", req.target_user_id);
        let _: () = conn
            .sadd(&key, visitor.id.to_string())
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("sadd presence: {e}")))?;
        // Refresh the TTL on every pulse so active visitors keep the set alive.
        let _: () = conn
            .expire(&key, 60)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("expire presence: {e}")))?;

        // Notify the target via the live WebSocket so their UI shows the aura.
        let event = serde_json::json!({
            "type": "presence:pulse",
            "from_user_id": visitor.id,
            "from_username": visitor.username,
        });
        let _: () = conn
            .publish(format!("user:{}", req.target_user_id), event.to_string())
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("publish presence: {e}")))?;
    }

    // 5. Count how many users are currently Pulsing Now on this target.
    let count: u32 = conn
        .scard(format!("presence:{}", req.target_user_id))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("scard presence: {e}")))?;

    Ok(Json(PulsePresenceResponse {
        aura_visible,
        pulsing_now_count: count,
    }))
}

/// GET /presence/{user_id} — who is currently Pulsing Now on this user?
/// Only returns users whose resonance with the requester is > 50%.
pub async fn list_presence(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    axum::extract::Path(target_user_id): axum::extract::Path<Uuid>,
) -> AppResult<Json<Vec<PresenceEntry>>> {
    let requester = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let mut conn = state.redis.clone();
    let members: Vec<String> = conn
        .smembers(format!("presence:{}", target_user_id))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("smembers: {e}")))?;

    let mut out = Vec::new();
    for m in members {
        let uid: Uuid = match m.parse() {
            Ok(u) => u,
            Err(_) => continue,
        };
        // Filter: requester must have > 50% resonance with the pulsing user.
        let r = queries::get_resonance(&state.db, requester.id, uid)
            .await
            .map_err(AppError::Db)?;
        if r <= 50 {
            continue;
        }
        // Fetch the username.
        let user: Option<(String,)> = sqlx::query_as("SELECT username FROM users WHERE id = $1")
            .bind(uid)
            .fetch_optional(&state.db)
            .await
            .map_err(AppError::Db)?;
        if let Some((username,)) = user {
            out.push(PresenceEntry { user_id: uid, username });
        }
    }
    Ok(Json(out))
}

/// GET /traces — list the requesting user's recent Passing Traces.
pub async fn list_my_traces(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
) -> AppResult<Json<Vec<TraceOut>>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let rows = queries::list_recent_traces(&state.db, me.id, 50)
        .await
        .map_err(AppError::Db)?;

    // Resolve visitor usernames for named traces.
    let mut out = Vec::with_capacity(rows.len());
    for t in rows {
        let visitor = if let Some(vid) = t.visitor_id {
            let row: Option<(String,)> = sqlx::query_as("SELECT username FROM users WHERE id = $1")
                .bind(vid)
                .fetch_optional(&state.db)
                .await
                .map_err(AppError::Db)?;
            row.map(|(u,)| u)
        } else {
            None
        };
        out.push(TraceOut {
            id: t.id,
            kind: t.kind,
            visitor_username: visitor,
            created_at: t.created_at,
            expires_at: t.expires_at,
        });
    }
    Ok(Json(out))
}

#[derive(Debug, Serialize)]
pub struct TraceOut {
    pub id: Uuid,
    pub kind: String,
    pub visitor_username: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}
