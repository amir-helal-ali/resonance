// ============================================================================
// resonance-backend/src/handlers/search.rs
// Search across users, pulses (by author), and hashtags.
//
// Note: pulse bodies are encrypted client-side, so we CANNOT do full-text
// search on pulse content. We search by:
//   - username (case-insensitive substring)
//   - hashtag (exact match)
//   - pulse author username
//
// For full-text search on decrypted content, you'd need a TEE-based search
// index — out of scope for this implementation.
// ============================================================================

use axum::{
    extract::{Query, State},
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
pub struct SearchQuery {
    pub q: String,
    pub kind: Option<String>, // "users" | "hashtags" | "all"
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum SearchResult {
    User {
        user_id: Uuid,
        username: String,
        imprint_preview: String,
    },
    Hashtag {
        hashtag_id: Uuid,
        tag: String,
        pulse_count: i64,
    },
}

/// GET /search?q=...&kind=...&limit=...
pub async fn search(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Query(q): Query<SearchQuery>,
) -> AppResult<Json<Vec<SearchResult>>> {
    let _me = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;

    let query = q.q.trim();
    if query.is_empty() {
        return Ok(Json(vec![]));
    }
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let kind = q.kind.unwrap_or_else(|| "all".into());
    let mut results = Vec::new();

    // 1. Search users (if kind is "users" or "all").
    if kind == "users" || kind == "all" {
        let pattern = format!("%{}%", query);
        let user_rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            r#"
            SELECT id, username, imprint
            FROM users
            WHERE username ILIKE $1
              AND is_quarantined = false
            ORDER BY username
            LIMIT $2
            "#,
        )
        .bind(&pattern)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Db)?;

        for (uid, username, imprint) in user_rows {
            let preview: String = imprint.chars().take(80).collect();
            results.push(SearchResult::User {
                user_id: uid,
                username,
                imprint_preview: preview,
            });
        }
    }

    // 2. Search hashtags (if kind is "hashtags" or "all").
    if kind == "hashtags" || kind == "all" {
        let tag_pattern = if query.starts_with('#') {
            query[1..].to_string()
        } else {
            query.to_string()
        };
        let like = format!("%{}%", tag_pattern);
        let tag_rows: Vec<(Uuid, String, i64)> = sqlx::query_as(
            r#"
            SELECT h.id, h.tag, COUNT(ph.pulse_id)::bigint AS pulse_count
            FROM hashtags h
            LEFT JOIN pulse_hashtags ph ON ph.hashtag_id = h.id
            WHERE h.tag ILIKE $1
            GROUP BY h.id, h.tag
            ORDER BY pulse_count DESC, h.tag
            LIMIT $2
            "#,
        )
        .bind(&like)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Db)?;

        for (hid, tag, count) in tag_rows {
            results.push(SearchResult::Hashtag {
                hashtag_id: hid,
                tag,
                pulse_count: count,
            });
        }
    }

    Ok(Json(results))
}

/// GET /search/hashtag/:tag — list pulses with a specific hashtag.
pub async fn pulses_by_hashtag(
    State(state): State<AppState>,
    VerifiedPubKey(_pubkey): VerifiedPubKey,
    axum::extract::Path(tag): axum::extract::Path<String>,
) -> AppResult<Json<Vec<HashtagPulseOut>>> {
    let rows: Vec<(Uuid, Uuid, Vec<u8>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT p.id, p.author_id, p.ciphertext, p.created_at
        FROM pulses p
        JOIN pulse_hashtags ph ON ph.pulse_id = p.id
        JOIN hashtags h ON h.id = ph.hashtag_id
        WHERE h.tag = $1::citext
          AND p.lifecycle <> 'evaporated'
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
