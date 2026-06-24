-- ============================================================================
-- Resonance (صدى) — Migration 0002: Goals, Transient Jury, Interactions extras
-- ============================================================================

-- ----------------------------------------------------------------------------
-- goals — "أُفقي" goal candles
-- ----------------------------------------------------------------------------
-- Each user can publish N goals (their "horizon"). Other users light candles
-- to support them. Each candle is a tiny micro-donation of resonance, not
-- money — it's a social signal.
CREATE TABLE goals (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title               TEXT         NOT NULL CHECK (char_length(title) BETWEEN 1 AND 140),
    -- The candle count target. When `current >= target`, the goal is "lit".
    target              INTEGER      NOT NULL CHECK (target > 0),
    -- Number of candles lit so far.
    current             INTEGER      NOT NULL DEFAULT 0,
    is_lit              BOOLEAN      NOT NULL DEFAULT false,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    lit_at              TIMESTAMPTZ
);
CREATE INDEX idx_goals_user ON goals (user_id, created_at DESC);

-- ----------------------------------------------------------------------------
-- goal_candles — who lit which candle (prevents double-lighting)
-- ----------------------------------------------------------------------------
CREATE TABLE goal_candles (
    goal_id             UUID         NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    supporter_id        UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    lit_at              TIMESTAMPTZ  NOT NULL DEFAULT now(),
    PRIMARY KEY (goal_id, supporter_id)
);

-- ----------------------------------------------------------------------------
-- jury_panels — Transient Jury
-- ----------------------------------------------------------------------------
-- When AI moderation flags a pulse with very high toxicity (≥0.9), or when a
-- pulse receives N user-reports, a jury of 5 randomly-selected users is
-- summoned. They have 24h to vote. Quorum = 3 votes. Verdict = majority.
CREATE TYPE jury_verdict AS ENUM ('pending', 'uphold', 'release', 'expire');
CREATE TYPE jury_vote    AS ENUM ('uphold', 'release');

CREATE TABLE jury_panels (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    pulse_id            UUID         NOT NULL REFERENCES pulses(id) ON DELETE CASCADE,
    -- 5 summoned user ids.
    juror_ids           UUID[]       NOT NULL,
    -- Track which jurors have voted.
    votes               JSONB        NOT NULL DEFAULT '{}'::jsonb,
    -- Final verdict: 'pending' until quorum, then 'uphold' or 'release'.
    -- 'expire' means 24h passed without quorum.
    final_verdict       jury_verdict NOT NULL DEFAULT 'pending',
    summoned_at         TIMESTAMPTZ  NOT NULL DEFAULT now(),
    expires_at          TIMESTAMPTZ  NOT NULL DEFAULT (now() + INTERVAL '24 hours'),
    concluded_at        TIMESTAMPTZ
);
CREATE INDEX idx_jury_pending ON jury_panels (final_verdict, expires_at);

-- ----------------------------------------------------------------------------
-- pulse_reports — user-flagged content (drives jury summoning)
-- ----------------------------------------------------------------------------
CREATE TABLE pulse_reports (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    pulse_id            UUID         NOT NULL REFERENCES pulses(id) ON DELETE CASCADE,
    reporter_id         UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reason              TEXT         NOT NULL CHECK (char_length(reason) BETWEEN 1 AND 500),
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    UNIQUE (pulse_id, reporter_id)
);
CREATE INDEX idx_reports_pulse ON pulse_reports (pulse_id);

-- ----------------------------------------------------------------------------
-- Add a `username` index to speed up the moderator's `find_user_by_pubkey`
-- reverse lookup (we already had `users.username` unique, so this is redundant
-- — skip).
-- ----------------------------------------------------------------------------

-- ----------------------------------------------------------------------------
-- sessions — short-lived signed session tokens
-- ----------------------------------------------------------------------------
-- Optional: the registration flow returns a `session_token` after OTP verify.
-- This table tracks issued tokens (with TTL 7 days) so we can revoke them.
-- Ed25519 signature verification is the primary auth mechanism; this is only
-- for the bootstrap period before the user has unlocked their keypair.
CREATE TABLE sessions (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- The token is opaque; we only store its SHA-256 hash (never the raw).
    token_hash          BYTEA        NOT NULL UNIQUE,
    issued_at           TIMESTAMPTZ  NOT NULL DEFAULT now(),
    expires_at          TIMESTAMPTZ  NOT NULL DEFAULT (now() + INTERVAL '7 days'),
    revoked_at          TIMESTAMPTZ
);
CREATE INDEX idx_sessions_user ON sessions (user_id) WHERE revoked_at IS NULL;
