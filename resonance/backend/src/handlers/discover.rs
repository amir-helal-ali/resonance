// ============================================================================
// resonance-backend/src/handlers/discover.rs
// Trending hashtags, suggested users, badges.
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

#[derive(Debug, Serialize)]
pub struct TrendingHashtag {
    pub hashtag_id: Uuid,
    pub tag: String,
    pub pulse_count: i64,
    pub unique_authors: i64,
}

/// GET /discover/trending — top 20 hashtags in the last 24h.
pub async fn trending_hashtags(
    State(state): State<AppState>,
    VerifiedPubKey(_pubkey): VerifiedPubKey,
) -> AppResult<Json<Vec<TrendingHashtag>>> {
    // Refresh the materialized view (cheap — it's already indexed).
    sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY trending_hashtags_24h")
        .execute(&state.db)
        .await
        .map_err(AppError::Db)?;

    let rows: Vec<(Uuid, String, i64, i64)> = sqlx::query_as(
        r#"
        SELECT hashtag_id, tag, pulse_count, unique_authors
        FROM trending_hashtags_24h
        ORDER BY unique_authors DESC, pulse_count DESC
        LIMIT 20
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    let out: Vec<TrendingHashtag> = rows
        .into_iter()
        .map(|r| TrendingHashtag {
            hashtag_id: r.0,
            tag: r.1,
            pulse_count: r.2,
            unique_authors: r.3,
        })
        .collect();
    Ok(Json(out))
}

#[derive(Debug, Serialize)]
pub struct SuggestedUser {
    pub user_id: Uuid,
    pub username: String,
    pub imprint_preview: String,
    pub jaccard: f32,
    pub badges: Vec<String>,
}

/// GET /discover/suggested-users — top 10 users with high Co-Resonance.
/// Reuses the connections::suggest logic but adds badges.
pub async fn suggested_users(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
) -> AppResult<Json<Vec<SuggestedUser>>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    // Use the same Jaccard logic as connections::suggest.
    let mine: Vec<Uuid> = sqlx::query_scalar(
        "SELECT target_id FROM connections WHERE source_id = $1 AND resonance_score >= 30",
    )
    .bind(me.id)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    if mine.is_empty() {
        // Fallback: random verified users.
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            r#"
            SELECT u.id, u.username, u.imprint
            FROM users u
            LEFT JOIN user_badges b ON b.user_id = u.id AND b.kind = 'verified'
            WHERE u.id <> $1 AND u.is_quarantined = false AND u.email_verified = true
            ORDER BY (b.user_id IS NOT NULL) DESC, random()
            LIMIT 10
            "#,
        )
        .bind(me.id)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Db)?;
        return Ok(Json(
            rows.into_iter()
                .map(|r| SuggestedUser {
                    user_id: r.0,
                    username: r.1,
                    imprint_preview: r.2.chars().take(80).collect(),
                    jaccard: 0.0,
                    badges: vec!["verified".into()],
                })
                .collect(),
        ));
    }

    let mine_set: std::collections::HashSet<Uuid> = mine.iter().copied().collect();
    let mut candidate_counts: std::collections::HashMap<Uuid, std::collections::HashSet<Uuid>> =
        std::collections::HashMap::new();
    for u in &mine {
        let theirs: Vec<Uuid> = sqlx::query_scalar(
            "SELECT target_id FROM connections WHERE source_id = $1 AND resonance_score >= 30",
        )
        .bind(u)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Db)?;
        for t in theirs {
            if t == me.id || mine_set.contains(&t) {
                continue;
            }
            candidate_counts.entry(t).or_default().insert(*u);
        }
    }

    let mut scored: Vec<(Uuid, f32)> = candidate_counts
        .into_iter()
        .map(|(cand, supporters)| {
            let inter = supporters.intersection(&mine_set).count() as f32;
            let union = supporters.union(&mine_set).count() as f32;
            (cand, if union > 0.0 { inter / union } else { 0.0 })
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(10);

    let mut out = Vec::with_capacity(scored.len());
    for (uid, j) in scored {
        let row: Option<(String, String)> = sqlx::query_as("SELECT username, imprint FROM users WHERE id = $1")
            .bind(uid)
            .fetch_optional(&state.db)
            .await
            .map_err(AppError::Db)?;
        if let Some((username, imprint)) = row {
            // Fetch badges.
            let badges: Vec<(String,)> = sqlx::query_as(
                "SELECT kind::text FROM user_badges WHERE user_id = $1",
            )
            .bind(uid)
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Db)?;
            out.push(SuggestedUser {
                user_id: uid,
                username,
                imprint_preview: imprint.chars().take(80).collect(),
                jaccard: j,
                badges: badges.into_iter().map(|(b,)| b).collect(),
            });
        }
    }
    Ok(Json(out))
}

/// GET /discover/hashtag/:tag — get pulses with this hashtag.
pub async fn hashtag_pulses(
    State(state): State<AppState>,
    VerifiedPubKey(_pubkey): VerifiedPubKey,
    Path(tag): Path<String>,
) -> AppResult<Json<Vec<HashtagPulseOut>>> {
    let rows: Vec<(Uuid, Uuid, Vec<u8>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT p.id, p.author_id, p.ciphertext, p.created_at
        FROM pulses p
        JOIN pulse_hashtags ph ON ph.pulse_id = p.id
        JOIN hashtags h ON h.id = ph.hashtag_id
        WHERE h.tag = $1::citext AND p.lifecycle <> 'evaporated'
        ORDER BY p.created_at DESC
        LIMIT 50
        "#,
    )
    .bind(&tag)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    let out: Vec<HashtagPulseOut> = rows
        .into_iter()
        .map(|(id, author_id, ciphertext, created_at)| HashtagPulseOut {
            pulse_id: id,
            author_id,
            ciphertext_b64: base64::encode(&ciphertext),
            created_at,
        })
        .collect();
    Ok(Json(out))
}

#[derive(Debug, Serialize)]
pub struct HashtagPulseOut {
    pub pulse_id: Uuid,
    pub author_id: Uuid,
    pub ciphertext_b64: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ----------------------------------------------------------------------------
// Notification preferences
// ----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct UpdateNotifPrefsRequest {
    /// Map of kind → bool (true = enabled, false = disabled).
    /// Missing keys = unchanged.
    pub disabled_kinds: Option<serde_json::Value>,
    pub push_enabled: Option<bool>,
    pub push_endpoint: Option<String>,
    pub push_p256dh: Option<String>,
    pub push_auth: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NotifPrefsOut {
    pub disabled_kinds: serde_json::Value,
    pub push_enabled: bool,
    pub push_endpoint: Option<String>,
}

/// GET /settings/notifications — get my notification preferences.
pub async fn get_notif_prefs(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
) -> AppResult<Json<NotifPrefsOut>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let row: Option<(serde_json::Value, bool, Option<String>)> = sqlx::query_as(
        "SELECT disabled_kinds, push_enabled, push_endpoint FROM notification_preferences WHERE user_id = $1",
    )
    .bind(me.id)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;

    let prefs = match row {
        Some((dk, pe, pend)) => NotifPrefsOut {
            disabled_kinds: dk,
            push_enabled: pe,
            push_endpoint: pend,
        },
        None => NotifPrefsOut {
            disabled_kinds: serde_json::json!({}),
            push_enabled: false,
            push_endpoint: None,
        },
    };
    Ok(Json(prefs))
}

/// PATCH /settings/notifications — update my notification preferences.
pub async fn update_notif_prefs(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Json(req): Json<UpdateNotifPrefsRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let dk = req.disabled_kinds.unwrap_or_else(|| serde_json::json!({}));
    let pe = req.push_enabled.unwrap_or(false);

    sqlx::query(
        r#"
        INSERT INTO notification_preferences
            (user_id, disabled_kinds, push_enabled, push_endpoint, push_p256dh, push_auth)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (user_id) DO UPDATE SET
            disabled_kinds = EXCLUDED.disabled_kinds,
            push_enabled   = EXCLUDED.push_enabled,
            push_endpoint  = EXCLUDED.push_endpoint,
            push_p256dh    = EXCLUDED.push_p256dh,
            push_auth      = EXCLUDED.push_auth,
            updated_at     = now()
        "#,
    )
    .bind(me.id)
    .bind(&dk)
    .bind(pe)
    .bind(&req.push_endpoint)
    .bind(&req.push_p256dh)
    .bind(&req.push_auth)
    .execute(&state.db)
    .await
    .map_err(AppError::Db)?;

    Ok(Json(serde_json::json!({ "updated": true })))
}

// ----------------------------------------------------------------------------
// Username lookup
// ----------------------------------------------------------------------------

/// GET /users/by-username/:username — fetch a user's public profile by username.
pub async fn lookup_by_username(
    State(state): State<AppState>,
    VerifiedPubKey(_pubkey): VerifiedPubKey,
    Path(username): Path<String>,
) -> AppResult<Json<UserProfileOut>> {
    let row: Option<(Uuid, String, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, username, imprint, horizon, created_at FROM users WHERE username = $1::citext",
    )
    .bind(&username)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let (id, uname, imprint, horizon, created_at) = row.ok_or(AppError::NotFound)?;

    // Fetch badges.
    let badges: Vec<(String,)> = sqlx::query_as("SELECT kind::text FROM user_badges WHERE user_id = $1")
        .bind(id)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Db)?;

    Ok(Json(UserProfileOut {
        user_id: id,
        username: uname,
        imprint,
        horizon,
        created_at,
        badges: badges.into_iter().map(|(b,)| b).collect(),
    }))
}

#[derive(Debug, Serialize)]
pub struct UserProfileOut {
    pub user_id: Uuid,
    pub username: String,
    pub imprint: String,
    pub horizon: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub badges: Vec<String>,
}
