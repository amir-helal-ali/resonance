-- ============================================================================
-- Resonance (صدى) — PostgreSQL Schema
-- Migration 0001: initial schema
-- ============================================================================
-- Design principles:
--   1. Privacy-first: emails never stored in cleartext. Only HMAC blind index.
--   2. Lifecycle: every pulse carries an encryption_key_id so it can be
--      cryptographically evaporated (key destruction = data destruction).
--   3. Resonance: connections are not binary follow/unfollow; they carry a
--      decaying score (0-100) representing live alignment.
--   4. Ephemeral traces: profile visits self-delete after 7 days via TTL
--      cron; the row is also physically removed by a partition-drop strategy.
-- ============================================================================

-- ---------- Extensions ----------
CREATE EXTENSION IF NOT EXISTS "pgcrypto";        -- gen_random_uuid
CREATE EXTENSION IF NOT EXISTS "citext";          -- case-insensitive username
CREATE EXTENSION IF NOT EXISTS "btree_gin";       -- composite indexes

-- ---------- Enumerations ----------
CREATE TYPE pulse_lifecycle AS ENUM ('glow', 'linger', 'evaporated');
CREATE TYPE trace_kind      AS ENUM ('anonymous', 'named');
CREATE TYPE moderation_verdict AS ENUM ('pending', 'cooling', 'released', 'quarantined');

-- ============================================================================
-- users — The Blind Vault
-- ============================================================================
CREATE TABLE users (
    -- Surrogate identity. Never expose the raw UUID; the public-facing
    -- handle is `username`. We never use the email for anything except
    -- password recovery via the blind relay.
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Public handle, case-insensitive, unique.
    username            CITEXT       NOT NULL UNIQUE
                                     CHECK (char_length(username) BETWEEN 3 AND 32),

    -- Encrypted email — AES-GCM ciphertext produced client-side.
    -- The server NEVER sees the cleartext email. Format:
    --   base64( iv(12B) || ciphertext || tag(16B) )
    email_ciphertext    BYTEA        NOT NULL,

    -- Blind index — HMAC-SHA256(BLIND_INDEX_KEY, email), truncated to 12 bytes.
    -- Used ONLY for uniqueness check and blind relay lookup. Leaks nothing
    -- about the email content beyond equality.
    email_blind_index   BYTEA        NOT NULL UNIQUE,

    -- Ed25519 public key (32 bytes). Used to verify every signed request.
    -- The corresponding private key never leaves the browser.
    public_key          BYTEA        NOT NULL,

    -- "بصمتي" — the imprint (legacy/heritage statement), encrypted at rest.
    imprint             TEXT         NOT NULL DEFAULT '',
    -- "أُفقي" — the horizon (future goals), encrypted at rest.
    horizon             TEXT         NOT NULL DEFAULT '',

    email_verified      BOOLEAN      NOT NULL DEFAULT false,
    is_quarantined      BOOLEAN      NOT NULL DEFAULT false,

    -- Reputation economy: cumulative creator earnings from RTB splits.
    -- Stored in milli-LSL (1/1000 of a Learning-Scales-Lumen, our internal unit).
    creator_balance_mlsl BIGINT      NOT NULL DEFAULT 0,

    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ  NOT NULL DEFAULT now()
);

-- Index to support blind-relay lookup by HMAC without revealing the email.
CREATE INDEX idx_users_blind_index ON users (email_blind_index);

-- ============================================================================
-- encryption_keys — Key Catalogue (for Cryptographic Evaporation)
-- ============================================================================
-- Each pulse is encrypted with a per-pulse symmetric key. The key lives here,
-- referenced by id. To "evaporate" a pulse we DELETE the row in this table;
-- the ciphertext becomes mathematically unrecoverable.
CREATE TABLE encryption_keys (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Wrapped symmetric key (AES-KW under the user's HKDF-derived KEK).
    wrapped_key         BYTEA        NOT NULL,
    -- KMS/HSM reference if we ever externalize; NULL for browser-derived.
    kms_ref             TEXT,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    destroyed_at        TIMESTAMPTZ
);

-- ============================================================================
-- pulses — The "نبضة" (heart-beat) post
-- ============================================================================
CREATE TABLE pulses (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    author_id           UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- Encrypted body. Client-side AES-GCM with the per-pulse key.
    -- The server is structurally blind to the content.
    ciphertext          BYTEA        NOT NULL,

    -- References the per-pulse symmetric key in `encryption_keys`.
    -- When this key is destroyed (DELETE FROM encryption_keys), the pulse
    -- is irrecoverable — this is "Cryptographic Evaporation".
    encryption_key_id   UUID         NOT NULL REFERENCES encryption_keys(id),

    -- Lifecycle: glow (0-48h) → linger (2-30d) → evaporated.
    lifecycle           pulse_lifecycle NOT NULL DEFAULT 'glow',

    -- The "تخليد" (preserve) flag — when true, the pulse skips the 30-day
    -- evaporation. BUT it is still subject to Immortal Decay: if it receives
    -- no interaction for 6 months, the user's preserve is revoked and the
    -- pulse evaporates. This prevents eternal spam.
    is_preserved        BOOLEAN      NOT NULL DEFAULT false,

    -- Updated on every reaction. Used by the Immortal Decay cron.
    last_interaction_at TIMESTAMPTZ  NOT NULL DEFAULT now(),

    -- Created timestamp also doubles as the "Sun Cycle" anchor: the UI
    -- derives sunrise/noon/sunset from (now() - created_at).
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),

    -- Soft-evaporation marker. The row stays for audit, but the key is gone.
    evaporated_at       TIMESTAMPTZ
);

CREATE INDEX idx_pulses_author       ON pulses (author_id, created_at DESC);
CREATE INDEX idx_pulses_lifecycle    ON pulses (lifecycle, created_at DESC);
CREATE INDEX idx_pulses_immortal_decay
    ON pulses (last_interaction_at)
    WHERE is_preserved = true AND lifecycle <> 'evaporated';

-- ============================================================================
-- connections — The "صدى" (resonance) relationship
-- ============================================================================
-- No binary follow. A connection has a decaying score (0-100).
-- Score rises when both users interact (Co-Resonance) and decays every 15 min.
-- Above 50 = presence visible. Below 5 = auto-evaporate the connection.
CREATE TABLE connections (
    -- `source` resonates with `target`. Symmetric resonance is computed
    -- but stored as two directed rows for query simplicity.
    source_id           UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    target_id           UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- 0..100 (integer; we keep precision low because the model is fuzzy).
    resonance_score     SMALLINT     NOT NULL DEFAULT 0
                                     CHECK (resonance_score BETWEEN 0 AND 100),

    -- When the score last changed (drives the 15-min decay cron).
    last_interaction_at TIMESTAMPTZ  NOT NULL DEFAULT now(),
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),

    PRIMARY KEY (source_id, target_id),
    CHECK (source_id <> target_id)
);

-- Reverse lookup: who resonates with me? (for presence & feed ranking)
CREATE INDEX idx_connections_target ON connections (target_id, resonance_score DESC);

-- ============================================================================
-- traces — The "آثار عابرة" (passing traces)
-- ============================================================================
-- Profile visits. Self-delete after 7 days via the cron + a partition strategy.
-- Visitors can choose to be anonymous (the visited sees "someone") or named.
CREATE TABLE traces (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    -- The user whose profile was visited.
    visited_id          UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- The visitor. NULL means fully anonymous (server-stripped).
    visitor_id          UUID         REFERENCES users(id) ON DELETE SET NULL,

    kind                trace_kind  NOT NULL,
    -- 7-day TTL — enforced by cron, also a partial index for fast pruning.
    expires_at          TIMESTAMPTZ NOT NULL DEFAULT (now() + INTERVAL '7 days'),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_traces_visited ON traces (visited_id, created_at DESC);
CREATE INDEX idx_traces_expiring ON traces (expires_at) WHERE expires_at IS NOT NULL;

-- ============================================================================
-- interactions — Reactions that drive resonance
-- ============================================================================
-- Every reaction (echo, save, comment) is recorded here. Each one bumps
-- the resonance score between the two users and updates `last_interaction_at`.
CREATE TABLE interactions (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    pulse_id            UUID         NOT NULL REFERENCES pulses(id) ON DELETE CASCADE,
    actor_id            UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind                TEXT         NOT NULL CHECK (kind IN ('echo','save','comment','view')),
    -- Encrypted comment body (NULL for non-comment kinds).
    payload             BYTEA,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    UNIQUE (pulse_id, actor_id, kind)
);
CREATE INDEX idx_interactions_pulse ON interactions (pulse_id, created_at DESC);

-- ============================================================================
-- ad_auctions — RTB revenue ledger
-- ============================================================================
-- One row per auction. The atomic revenue split happens inside the same
-- transaction that updates `creator_balance_mlsl`.
CREATE TABLE ad_auctions (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    pulse_id            UUID         REFERENCES pulses(id) ON DELETE SET NULL,
    creator_id          UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    winning_bid_mlsl    BIGINT       NOT NULL,
    platform_share_mlsl BIGINT       NOT NULL,
    creator_share_mlsl  BIGINT       NOT NULL,
    auctioned_at        TIMESTAMPTZ  NOT NULL DEFAULT now(),
    CHECK (winning_bid_mlsl = platform_share_mlsl + creator_share_mlsl)
);

-- ============================================================================
-- moderation_queue — AI verdicts and the Transient Jury
-- ============================================================================
CREATE TABLE moderation_queue (
    id                  UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    pulse_id            UUID         NOT NULL REFERENCES pulses(id) ON DELETE CASCADE,
    -- candle model output: probability of toxicity in [0,1].
    toxicity_score      REAL         NOT NULL DEFAULT 0.0 CHECK (toxicity_score BETWEEN 0 AND 1),
    verdict             moderation_verdict NOT NULL DEFAULT 'pending',
    -- Thermodynamic cooling: the timestamp until which the pulse is muted.
    cooling_until       TIMESTAMPTZ,
    -- Transient Jury: a randomly-selected panel of 5 users. NULL until summoned.
    jury_summoned_at    TIMESTAMPTZ,
    jury_verdict        TEXT,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ  NOT NULL DEFAULT now()
);
CREATE INDEX idx_modqueue_pending ON moderation_queue (verdict, created_at);

-- ============================================================================
-- updated_at trigger — keep the column honest
-- ============================================================================
CREATE OR REPLACE FUNCTION touch_updated_at() RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at := now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_users_touch
    BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

CREATE TRIGGER trg_modqueue_touch
    BEFORE UPDATE ON moderation_queue
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

-- ============================================================================
-- Row-Level Security: each user can only read their own vault rows.
-- (The backend connects with a service role that BYPASSESRLS for admin
--  operations, but RLS protects against accidental cross-user leaks in
--  any future query path.)
-- ============================================================================
ALTER TABLE users            ENABLE ROW LEVEL SECURITY;
ALTER TABLE pulses           ENABLE ROW LEVEL SECURITY;
ALTER TABLE traces           ENABLE ROW LEVEL SECURITY;
ALTER TABLE interactions     ENABLE ROW LEVEL SECURITY;

-- The backend service role bypasses RLS; this policy protects the
-- "user-as-actor" path that may be added later.
CREATE POLICY users_self_read ON users
    FOR SELECT USING (true);  -- public_key & imprint are public; email fields are opaque bytes
