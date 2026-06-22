// ============================================================================
// resonance-backend/src/handlers/goals.rs
// "أُفقي" (horizon) goals + "شموع الدعم" (candles).
//
// A user publishes goals (their horizon). Other users light candles to
// support them. Lighting a candle:
//   1. Records (goal_id, supporter_id) — dedupes.
//   2. Increments `goals.current`.
//   3. When `current >= target`, sets `is_lit=true` and `lit_at=now()`.
//   4. Bumps resonance supporter → goal owner by 3 points.
// ============================================================================

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::queries,
    errors::{AppError, AppResult},
    middleware::signature::VerifiedPubKey,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct CreateGoalRequest {
    pub title: String,
    pub target: i32,
}

#[derive(Debug, Serialize)]
pub struct GoalOut {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub target: i32,
    pub current: i32,
    pub is_lit: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub lit_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct LightCandleResponse {
    pub new_current: i32,
    pub is_lit: bool,
    pub new_resonance_score: i16,
}

/// POST /goals — create a goal.
pub async fn create_goal(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Json(req): Json<CreateGoalRequest>,
) -> AppResult<Json<GoalOut>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    if req.title.len() < 1 || req.title.len() > 140 {
        return Err(AppError::BadRequest("title length 1..140".into()));
    }
    if req.target <= 0 || req.target > 10_000 {
        return Err(AppError::BadRequest("target 1..10000".into()));
    }

    let row: (Uuid, Uuid, String, i32, i32, bool, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        r#"
        INSERT INTO goals (user_id, title, target)
        VALUES ($1, $2, $3)
        RETURNING id, user_id, title, target, current, is_lit, created_at, lit_at
        "#,
    )
    .bind(me.id)
    .bind(&req.title)
    .bind(req.target)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Db)?;

    Ok(Json(GoalOut {
        id: row.0,
        user_id: row.1,
        title: row.2,
        target: row.3,
        current: row.4,
        is_lit: row.5,
        created_at: row.6,
        lit_at: row.7,
    }))
}

/// GET /goals/:user_id — list a user's goals.
pub async fn list_goals(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> AppResult<Json<Vec<GoalOut>>> {
    let rows: Vec<(Uuid, Uuid, String, i32, i32, bool, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
        r#"
        SELECT id, user_id, title, target, current, is_lit, created_at, lit_at
        FROM goals
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    let out: Vec<GoalOut> = rows
        .into_iter()
        .map(|r| GoalOut {
            id: r.0,
            user_id: r.1,
            title: r.2,
            target: r.3,
            current: r.4,
            is_lit: r.5,
            created_at: r.6,
            lit_at: r.7,
        })
        .collect();
    Ok(Json(out))
}

/// POST /goals/:id/light — light a candle on a goal.
pub async fn light_candle(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Path(goal_id): Path<Uuid>,
) -> AppResult<Json<LightCandleResponse>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    // 1. Insert the candle row (idempotent — UNIQUE constraint).
    let inserted: Option<(Uuid,)> = sqlx::query_as(
        r#"
        INSERT INTO goal_candles (goal_id, supporter_id)
        VALUES ($1, $2)
        ON CONFLICT (goal_id, supporter_id) DO NOTHING
        RETURNING goal_id
        "#,
    )
    .bind(goal_id)
    .bind(me.id)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;

    let (new_current, is_lit, owner_id): (i32, bool, Uuid) = if inserted.is_some() {
        // 2a. Atomically increment + check if lit.
        let row: (i32, bool, Uuid) = sqlx::query_as(
            r#"
            UPDATE goals
            SET current = current + 1,
                is_lit  = (current + 1 >= target),
                lit_at  = CASE WHEN (current + 1 >= target) AND lit_at IS NULL
                              THEN now() ELSE lit_at END
            WHERE id = $1
            RETURNING current, is_lit, user_id
            "#,
        )
        .bind(goal_id)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Db)?;
        row
    } else {
        // 2b. Already lit by this user — return current state.
        let row: (i32, bool, Uuid) = sqlx::query_as(
            "SELECT current, is_lit, user_id FROM goals WHERE id = $1",
        )
        .bind(goal_id)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Db)?;
        row
    };

    // 3. Bump resonance supporter → owner (if not self).
    let new_resonance_score = if owner_id != me.id && inserted.is_some() {
        let conn = queries::upsert_connection(&state.db, me.id, owner_id, 3)
            .await
            .map_err(AppError::Db)?;
        conn.resonance_score
    } else {
        queries::get_resonance(&state.db, me.id, owner_id)
            .await
            .map_err(AppError::Db)?
    };

    Ok(Json(LightCandleResponse {
        new_current,
        is_lit,
        new_resonance_score,
    }))
}
