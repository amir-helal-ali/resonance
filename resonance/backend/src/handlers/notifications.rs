// ============================================================================
// resonance-backend/src/handlers/notifications.rs
// The notification center. Users see:
//   - echoes, saves, comments on their pulses
//   - new syncs (followers)
//   - resonance threshold crossings
//   - mentions
//   - jury summons + verdicts
//   - goal candles lit
//   - profile traces
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
pub struct ListNotificationsQuery {
    pub unread_only: Option<bool>,
    pub limit: Option<i64>,
    pub before: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct NotificationOut {
    pub id: Uuid,
    pub kind: String,
    pub actor_id: Option<Uuid>,
    pub actor_username: Option<String>,
    pub target_type: Option<String>,
    pub target_id: Option<Uuid>,
    pub payload: serde_json::Value,
    pub read_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// GET /notifications — list my notifications.
pub async fn list_notifications(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Query(q): Query<ListNotificationsQuery>,
) -> AppResult<Json<Vec<NotificationOut>>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let unread_only = q.unread_only.unwrap_or(false);

    let rows: Vec<(Uuid, String, Option<Uuid>, Option<String>, Option<Uuid>, serde_json::Value, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT n.id, n.kind::text, n.actor_id, u.username, n.target_id, n.payload, n.read_at, n.created_at
        FROM notifications n
        LEFT JOIN users u ON u.id = n.actor_id
        WHERE n.user_id = $1
          AND ($2::bool = false OR n.read_at IS NULL)
          AND ($3::timestamptz IS NULL OR n.created_at < $3)
        ORDER BY n.created_at DESC
        LIMIT $4
        "#,
    )
    .bind(me.id)
    .bind(unread_only)
    .bind(q.before)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    let out: Vec<NotificationOut> = rows
        .into_iter()
        .map(|r| NotificationOut {
            id: r.0,
            kind: r.1,
            actor_id: r.2,
            actor_username: r.3,
            target_type: None, // we'd need another column or derive from kind
            target_id: r.4,
            payload: r.5,
            read_at: r.6,
            created_at: r.7,
        })
        .collect();
    Ok(Json(out))
}

/// POST /notifications/:id/read — mark a single notification as read.
pub async fn mark_read(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Path(notif_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    sqlx::query("UPDATE notifications SET read_at = now() WHERE id = $1 AND user_id = $2")
        .bind(notif_id)
        .bind(me.id)
        .execute(&state.db)
        .await
        .map_err(AppError::Db)?;

    Ok(Json(serde_json::json!({ "read": notif_id })))
}

/// POST /notifications/read-all — mark all my notifications as read.
pub async fn mark_all_read(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
) -> AppResult<Json<serde_json::Value>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let res = sqlx::query(
        "UPDATE notifications SET read_at = now() WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(me.id)
    .execute(&state.db)
    .await
    .map_err(AppError::Db)?;

    Ok(Json(serde_json::json!({ "marked_read": res.rows_affected() })))
}

/// GET /notifications/unread-count — quick count for the navbar badge.
pub async fn unread_count(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
) -> AppResult<Json<serde_json::Value>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(me.id)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Db)?;

    Ok(Json(serde_json::json!({ "unread": count })))
}
