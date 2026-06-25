-- ============================================================================
-- Resonance (صدى) — Migration 0003: DMs, Notifications, Blocks, Hashtags, Saved
-- ============================================================================

-- ----------------------------------------------------------------------------
-- direct_messages — E2E encrypted DMs
-- ----------------------------------------------------------------------------
-- Each DM is encrypted client-side with a per-conversation key derived via
-- X25519 ECDH between the two users' Ed25519 keys (converted to X25519).
-- The server stores ONLY ciphertext. DMs auto-expire after 30 days.
CREATE TABLE direct_messages (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    -- The "conversation" is identified by the lexicographically-smaller user id first.
    -- This makes querying a conversation a single index lookup.
    user_a              UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_b              UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    sender_id           UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- AES-GCM ciphertext of the message body.
    ciphertext          BYTEA        NOT NULL,
    -- Ephemeral X25519 public key used by the sender for this message.
    ephemeral_pubkey    BYTEA        NOT NULL,
    read_at             TIMESTAMPTZ,
    expires_at          TIMESTAMPTZ  NOT NULL DEFAULT (now() + INTERVAL '30 days'),
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    CHECK (user_a < user_b),  -- canonical ordering: user_a is always the smaller UUID
    CHECK (sender_id = user_a OR sender_id = user_b)
);
CREATE INDEX idx_dms_conversation ON direct_messages (user_a, user_b, created_at DESC);
CREATE INDEX idx_dms_expiring     ON direct_messages (expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX idx_dms_unread       ON direct_messages (user_b, sender_id, read_at)
    WHERE read_at IS NULL;

-- ----------------------------------------------------------------------------
-- notifications — Real-time notification feed
-- ----------------------------------------------------------------------------
CREATE TYPE notification_kind AS ENUM (
    'echo',           -- someone echoed your pulse
    'save',           -- someone saved your pulse
    'comment',        -- someone commented on your pulse
    'sync',           -- someone synced (followed) you
    'resonance',      -- your resonance crossed a threshold
    'mention',        -- you were mentioned in a pulse
    'jury_summon',    -- you were summoned as a juror
    'jury_verdict',   -- a jury panel you're on concluded
    'goal_lit',       -- your goal was lit
    'candle',         -- someone lit your goal candle
    'trace'           -- someone visited your profile
);

CREATE TABLE notifications (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind                notification_kind NOT NULL,
    -- The user who triggered the notification (NULL for system notifications).
    actor_id            UUID         REFERENCES users(id) ON DELETE SET NULL,
    -- Polymorphic reference: pulse_id, goal_id, panel_id, etc.
    target_type         TEXT,        -- 'pulse' | 'goal' | 'jury_panel' | 'user'
    target_id           UUID,
    -- JSON blob for extra context (e.g. resonance_score, comment preview).
    payload             JSONB        NOT NULL DEFAULT '{}'::jsonb,
    read_at             TIMESTAMPTZ,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now()
);
CREATE INDEX idx_notifications_user ON notifications (user_id, created_at DESC)
    WHERE read_at IS NULL;
CREATE INDEX idx_notifications_user_all ON notifications (user_id, created_at DESC);

-- ----------------------------------------------------------------------------
-- blocks — User blocks (one-directional)
-- ----------------------------------------------------------------------------
-- A blocked user cannot: sync, DM, view profile (presence), or comment on
-- the blocker's pulses. Block is asymmetric: A blocks B does NOT mean B
-- blocks A.
CREATE TABLE blocks (
    blocker_id          UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    blocked_id          UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    PRIMARY KEY (blocker_id, blocked_id),
    CHECK (blocker_id <> blocked_id)
);

-- Mutes: same shape as blocks but lighter — muted users' pulses are
-- filtered from the blocker's feed but they can still DM/comment.
CREATE TABLE mutes (
    muter_id            UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    muted_id            UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    PRIMARY KEY (muter_id, muted_id),
    CHECK (muter_id <> muted_id)
);

-- ----------------------------------------------------------------------------
-- hashtags — Topic organization
-- ----------------------------------------------------------------------------
CREATE TABLE hashtags (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    tag                 CITEXT       NOT NULL UNIQUE
                                CHECK (char_length(tag) BETWEEN 1 AND 64),
    -- The first user to use this tag (for moderation audit).
    created_by          UUID         REFERENCES users(id) ON DELETE SET NULL,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now()
);
CREATE INDEX idx_hashtags_tag ON hashtags (tag);

-- ----------------------------------------------------------------------------
-- pulse_hashtags — Many-to-many join
-- ----------------------------------------------------------------------------
CREATE TABLE pulse_hashtags (
    pulse_id            UUID         NOT NULL REFERENCES pulses(id) ON DELETE CASCADE,
    hashtag_id          UUID         NOT NULL REFERENCES hashtags(id) ON DELETE CASCADE,
    PRIMARY KEY (pulse_id, hashtag_id)
);
CREATE INDEX idx_pulse_hashtags_tag ON pulse_hashtags (hashtag_id, pulse_id);

-- ----------------------------------------------------------------------------
-- saved_pulses — Bookmarks (separate from the `save` interaction)
-- ----------------------------------------------------------------------------
-- The `interactions` table records the act of saving (for resonance bump +
-- metrics). `saved_pulses` is the user's bookmark list (with ordering +
-- optional personal note). A user can save without leaving a public trace.
CREATE TABLE saved_pulses (
    user_id             UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    pulse_id            UUID         NOT NULL REFERENCES pulses(id) ON DELETE CASCADE,
    personal_note       TEXT         NOT NULL DEFAULT '',
    saved_at            TIMESTAMPTZ  NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, pulse_id)
);
CREATE INDEX idx_saved_user ON saved_pulses (user_id, saved_at DESC);

-- ----------------------------------------------------------------------------
-- profile_views — Aggregated view counter (for the profile owner's analytics)
-- ----------------------------------------------------------------------------
-- Unlike `traces` (which are individual visit records with 7d TTL), this
-- table stores aggregate counts per (profile_owner, day) for a 90-day
-- rolling window. Useful for the "إحصائيات" tab.
CREATE TABLE profile_views_daily (
    user_id             UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    view_date           DATE         NOT NULL,
    anonymous_views     INTEGER      NOT NULL DEFAULT 0,
    named_views         INTEGER      NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, view_date)
);
