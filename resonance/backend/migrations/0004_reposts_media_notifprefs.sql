-- ============================================================================
-- Resonance (صدى) — Migration 0004: reposts, media, notification prefs
-- ============================================================================

-- ----------------------------------------------------------------------------
-- reposts — Viral mechanic: a pulse can be reposted (with optional quote).
-- A repost is a NEW pulse that references the original; the repost's own
-- ciphertext is the (optional) quote comment. The original pulse's author
-- gets a notification + a resonance bump.
-- ----------------------------------------------------------------------------
CREATE TABLE reposts (
    repost_pulse_id     UUID         PRIMARY KEY REFERENCES pulses(id) ON DELETE CASCADE,
    original_pulse_id   UUID         NOT NULL REFERENCES pulses(id) ON DELETE CASCADE,
    is_quote            BOOLEAN      NOT NULL DEFAULT false,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    CHECK (repost_pulse_id <> original_pulse_id)
);
CREATE INDEX idx_reposts_original ON reposts (original_pulse_id, created_at DESC);

-- ----------------------------------------------------------------------------
-- media_attachments — Encrypted media (images, short videos) attached to pulses.
-- The media bytes are AES-GCM encrypted client-side with the SAME per-pulse
-- key used for the pulse body. The server stores opaque ciphertext + metadata.
-- ----------------------------------------------------------------------------
CREATE TYPE media_kind AS ENUM ('image', 'video', 'audio');

CREATE TABLE media_attachments (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    pulse_id            UUID         NOT NULL REFERENCES pulses(id) ON DELETE CASCADE,
    kind                media_kind   NOT NULL,
    -- MIME type of the ORIGINAL (pre-encryption) file, for client display.
    mime_type           TEXT         NOT NULL,
    -- AES-GCM ciphertext of the media bytes. Can be large; stored as bytea.
    ciphertext          BYTEA        NOT NULL,
    -- IV used for AES-GCM (12 bytes).
    iv                  BYTEA        NOT NULL,
    -- Width/height for images/videos (0 if N/A).
    width               INTEGER      NOT NULL DEFAULT 0,
    height              INTEGER      NOT NULL DEFAULT 0,
    -- Duration in milliseconds for video/audio (0 if N/A).
    duration_ms         INTEGER      NOT NULL DEFAULT 0,
    -- SHA-256 of the ORIGINAL file (for dedup detection; does NOT leak content).
    original_sha256     BYTEA        NOT NULL,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now()
);
CREATE INDEX idx_media_pulse ON media_attachments (pulse_id);

-- ----------------------------------------------------------------------------
-- notification_preferences — Per-user opt-outs for notification kinds.
-- ----------------------------------------------------------------------------
CREATE TABLE notification_preferences (
    user_id             UUID         PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    -- JSON object: { "echo": false, "save": true, "comment": true, ... }
    -- Missing keys = default (true).
    disabled_kinds      JSONB        NOT NULL DEFAULT '{}'::jsonb,
    -- Whether to send push notifications (via Web Push API).
    push_enabled        BOOLEAN      NOT NULL DEFAULT false,
    -- Web Push subscription endpoint (NULL if not subscribed).
    push_endpoint       TEXT,
    push_p256dh         TEXT,
    push_auth           TEXT,
    updated_at          TIMESTAMPTZ  NOT NULL DEFAULT now()
);

-- ----------------------------------------------------------------------------
-- muted_hashtags — Per-user hashtag mutes (hashtags they don't want in feed).
-- ----------------------------------------------------------------------------
CREATE TABLE muted_hashtags (
    user_id             UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    hashtag_id          UUID         NOT NULL REFERENCES hashtags(id) ON DELETE CASCADE,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, hashtag_id)
);

-- ----------------------------------------------------------------------------
-- user_followup_badges — Verification badges (مؤكد / صانع محتوى / خبير)
-- ----------------------------------------------------------------------------
CREATE TYPE badge_kind AS ENUM ('verified', 'creator', 'expert', 'founder');

CREATE TABLE user_badges (
    user_id             UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind                badge_kind   NOT NULL,
    awarded_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    awarded_by          UUID         REFERENCES users(id) ON DELETE SET NULL,
    PRIMARY KEY (user_id, kind)
);

-- ----------------------------------------------------------------------------
-- Add `original_sha256` to pulses for dedup detection (optional).
-- This lets the server detect "this pulse body was already posted by this
-- user" without decrypting — useful for spam prevention.
-- ----------------------------------------------------------------------------
ALTER TABLE pulses ADD COLUMN IF NOT EXISTS original_sha256 BYTEA;

-- ----------------------------------------------------------------------------
-- Trending: a materialized view of hashtag usage in the last 24h.
-- Refreshed by a cron job (job 9).
-- ----------------------------------------------------------------------------
CREATE MATERIALIZED VIEW IF NOT EXISTS trending_hashtags_24h AS
    SELECT
        h.id AS hashtag_id,
        h.tag,
        COUNT(DISTINCT ph.pulse_id) AS pulse_count,
        COUNT(DISTINCT p.author_id) AS unique_authors
    FROM hashtags h
    JOIN pulse_hashtags ph ON ph.hashtag_id = h.id
    JOIN pulses p ON p.id = ph.pulse_id
    WHERE p.created_at > now() - INTERVAL '24 hours'
      AND p.lifecycle <> 'evaporated'
    GROUP BY h.id, h.tag
    ORDER BY unique_authors DESC, pulse_count DESC
    LIMIT 100;

CREATE UNIQUE INDEX IF NOT EXISTS idx_trending_hashtags_24h_tag
    ON trending_hashtags_24h (hashtag_id);
