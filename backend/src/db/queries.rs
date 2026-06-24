// ============================================================================
// resonance-backend/src/db/queries.rs
// Typed query helpers. Each function is one logical operation.
// All queries use sqlx::query! with compile-time verification when
// SQLX_OFFLINE=true is set (the Dockerfile sets this).
// ============================================================================

use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::{ConnectionRow, ModerationRow, PulseRow, TraceRow, UserRow};

// ----------------------------------------------------------------------------
// users
// ----------------------------------------------------------------------------

pub async fn insert_user(
    pool: &PgPool,
    username: &str,
    email_ciphertext: &[u8],
    email_blind_index: &[u8],
    public_key: &[u8],
) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        r#"
        INSERT INTO users (username, email_ciphertext, email_blind_index, public_key)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
    )
    .bind(username)
    .bind(email_ciphertext)
    .bind(email_blind_index)
    .bind(public_key)
    .fetch_one(pool)
    .await
}

pub async fn find_user_by_username(pool: &PgPool, username: &str) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE username = $1")
        .bind(username)
        .fetch_optional(pool)
        .await
}

pub async fn find_user_by_id(pool: &PgPool, id: Uuid) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn set_email_verified(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET email_verified = true WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// ----------------------------------------------------------------------------
// pulses
// ----------------------------------------------------------------------------

pub async fn insert_pulse(
    pool: &PgPool,
    author_id: Uuid,
    ciphertext: &[u8],
    encryption_key_id: Uuid,
) -> Result<PulseRow, sqlx::Error> {
    sqlx::query_as::<_, PulseRow>(
        r#"
        INSERT INTO pulses (author_id, ciphertext, encryption_key_id, lifecycle)
        VALUES ($1, $2, $3, 'glow')
        RETURNING *
        "#,
    )
    .bind(author_id)
    .bind(ciphertext)
    .bind(encryption_key_id)
    .fetch_one(pool)
    .await
}

pub async fn mark_pulse_linger(pool: &PgPool, pulse_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE pulses SET lifecycle = 'linger'
        WHERE id = $1 AND lifecycle = 'glow'
        "#,
    )
    .bind(pulse_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn evaporate_pulse(pool: &PgPool, pulse_id: Uuid) -> Result<(), sqlx::Error> {
    // Two-step: destroy the key first, then mark the pulse evaporated.
    // If the second step fails, the key is still gone (data is unrecoverable),
    // and a retry will succeed.
    let pulse: PulseRow = sqlx::query_as::<_, PulseRow>(
        "SELECT * FROM pulses WHERE id = $1 FOR UPDATE",
    )
    .bind(pulse_id)
    .fetch_one(pool)
    .await?;

    sqlx::query("UPDATE encryption_keys SET destroyed_at = now() WHERE id = $1")
        .bind(pulse.encryption_key_id)
        .execute(pool)
        .await?;

    sqlx::query(
        r#"
        UPDATE pulses
        SET lifecycle = 'evaporated', evaporated_at = now(), is_preserved = false
        WHERE id = $1
        "#,
    )
    .bind(pulse_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Find pulses older than 30 days that are NOT preserved and not yet evaporated.
pub async fn find_pulses_to_evaporate(pool: &PgPool) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id FROM pulses
        WHERE lifecycle <> 'evaporated'
          AND is_preserved = false
          AND created_at < now() - INTERVAL '30 days'
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Find preserved pulses whose last interaction is older than 6 months.
/// These lose their "تخليد" status and get evaporated (Immortal Decay).
pub async fn find_immortal_decay_candidates(pool: &PgPool) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id FROM pulses
        WHERE lifecycle <> 'evaporated'
          AND is_preserved = true
          AND last_interaction_at < now() - INTERVAL '6 months'
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Promote pulses from `glow` to `linger` (i.e. older than 48h).
pub async fn promote_glow_to_linger(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        r#"
        UPDATE pulses SET lifecycle = 'linger'
        WHERE lifecycle = 'glow'
          AND created_at < now() - INTERVAL '48 hours'
        "#,
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// Touch `last_interaction_at` for a pulse (used by the interaction handler
/// to delay Immortal Decay).
pub async fn touch_pulse_interaction(pool: &PgPool, pulse_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE pulses SET last_interaction_at = now() WHERE id = $1")
        .bind(pulse_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ----------------------------------------------------------------------------
// connections (resonance)
// ----------------------------------------------------------------------------

pub async fn upsert_connection(
    pool: &PgPool,
    source_id: Uuid,
    target_id: Uuid,
    score_delta: i16,
) -> Result<ConnectionRow, sqlx::Error> {
    sqlx::query_as::<_, ConnectionRow>(
        r#"
        INSERT INTO connections (source_id, target_id, resonance_score, last_interaction_at)
        VALUES ($1, $2, LEAST($3::int, 100), now())
        ON CONFLICT (source_id, target_id) DO UPDATE
          SET resonance_score = LEAST(connections.resonance_score + $3, 100),
              last_interaction_at = now()
        RETURNING *
        "#,
    )
    .bind(source_id)
    .bind(target_id)
    .bind(score_delta as i32)
    .fetch_one(pool)
    .await
}

pub async fn decay_resonance(pool: &PgPool) -> Result<u64, sqlx::Error> {
    // Halve every 15 min for connections idle for >7 days. Below 5 = delete.
    let res = sqlx::query(
        r#"
        DELETE FROM connections
        WHERE last_interaction_at < now() - INTERVAL '7 days'
          AND resonance_score < 5;

        UPDATE connections
        SET resonance_score = GREATEST(resonance_score / 2, 0)
        WHERE last_interaction_at < now() - INTERVAL '7 days';
        "#,
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

pub async fn get_resonance(pool: &PgPool, source: Uuid, target: Uuid) -> Result<i16, sqlx::Error> {
    let row: Option<(i16,)> = sqlx::query_as(
        "SELECT resonance_score FROM connections WHERE source_id = $1 AND target_id = $2",
    )
    .bind(source)
    .bind(target)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(s,)| s).unwrap_or(0))
}

// ----------------------------------------------------------------------------
// traces (passing traces — 7-day TTL)
// ----------------------------------------------------------------------------

pub async fn insert_trace(
    pool: &PgPool,
    visited_id: Uuid,
    visitor_id: Option<Uuid>,
    kind: &str, // "anonymous" | "named"
) -> Result<TraceRow, sqlx::Error> {
    sqlx::query_as::<_, TraceRow>(
        r#"
        INSERT INTO traces (visited_id, visitor_id, kind, expires_at)
        VALUES ($1, $2, $3::trace_kind, now() + INTERVAL '7 days')
        RETURNING *
        "#,
    )
    .bind(visited_id)
    .bind(visitor_id)
    .bind(kind)
    .fetch_one(pool)
    .await
}

pub async fn prune_expired_traces(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let res = sqlx::query("DELETE FROM traces WHERE expires_at < now()")
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}

pub async fn list_recent_traces(
    pool: &PgPool,
    visited_id: Uuid,
    limit: i64,
) -> Result<Vec<TraceRow>, sqlx::Error> {
    sqlx::query_as::<_, TraceRow>(
        "SELECT * FROM traces WHERE visited_id = $1 ORDER BY created_at DESC LIMIT $2",
    )
    .bind(visited_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

// ----------------------------------------------------------------------------
// moderation
// ----------------------------------------------------------------------------

pub async fn insert_moderation_row(
    pool: &PgPool,
    pulse_id: Uuid,
    toxicity_score: f32,
) -> Result<ModerationRow, sqlx::Error> {
    let verdict = if toxicity_score >= 0.7 { "cooling" } else { "released" };
    let cooling_until = if toxicity_score >= 0.7 {
        // Thermodynamic cooling: the hotter the toxicity, the longer the cool.
        // 1 minute per 0.05 over 0.70, capped at 24h.
        let minutes = ((toxicity_score - 0.70) / 0.05 * 60.0).min(1440.0) as i32;
        Some(chrono::Utc::now() + chrono::Duration::minutes(minutes as i64))
    } else {
        None
    };
    sqlx::query_as::<_, ModerationRow>(
        r#"
        INSERT INTO moderation_queue (pulse_id, toxicity_score, verdict, cooling_until)
        VALUES ($1, $2, $3::moderation_verdict, $4)
        RETURNING *
        "#,
    )
    .bind(pulse_id)
    .bind(toxicity_score)
    .bind(verdict)
    .bind(cooling_until)
    .fetch_one(pool)
    .await
}
