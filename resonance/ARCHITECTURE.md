# Resonance (صدى) — Architecture Manifest

> A chrono-social, privacy-first platform. Backend in 100% Rust, frontend in SvelteKit.

```
                                ┌─────────────────────────────────────────┐
                                │              THE BROWSER                │
                                │                                         │
                                │  ┌────────────┐   ┌──────────────────┐  │
                                │  │ Web Crypto │   │  Web Workers     │  │
                                │  │   API      │   │  (PoW solver,    │  │
                                │  │            │   │   blind index)   │  │
                                │  └─────┬──────┘   └────────┬─────────┘  │
                                │        │                   │            │
                                │  ┌─────▼───────────────────▼─────────┐  │
                                │  │     SvelteKit Frontend            │  │
                                │  │  (TailwindCSS: papyrus /          │  │
                                │  │   astral-blue / egyptian-gold)    │  │
                                │  │                                   │  │
                                │  │  - Blind Vault Registration       │  │
                                │  │  - The Living Profile & Manifesto │  │
                                │  │  - Live Feed (Nile Flow)          │  │
                                │  │  - Odometer interaction counters   │  │
                                │  └───────────┬───────────┬───────────┘  │
                                │              │           │              │
                                │        HTTPS │    WSS    │              │
                                └──────────────┼───────────┼──────────────┘
                                               │           │
                              ╔════════════════╪═══════════╪══════════════╗
                              ║                ▼           ▼               ║
                              ║      ┌──────────────────────────────┐      ║
                              ║      │      Rust / Axum Backend     │      ║
                              ║      │       (the crypto core)      │      ║
                              ║      │                              │      ║
                              ║      │ ┌────────────┐ ┌───────────┐ │      ║
                              ║      │ │ Blind Vault│ │  Ed25519  │ │      ║
                              ║      │ │  Handler   │ │  Verify   │ │      ║
                              ║      │ │ (PoW+HMAC) │ │  MW       │ │      ║
                              ║      │ └─────┬──────┘ └─────┬─────┘ │      ║
                              ║      │       │              │       │      ║
                              ║      │ ┌─────▼──────────────▼─────┐ │      ║
                              ║      │ │    Lifecycle Scheduler   │ │      ║
                              ║      │ │  (tokio-cron-scheduler)  │ │      ║
                              ║      │ │                          │ │      ║
                              ║      │ │  • Evaporate (30d)       │ │      ║
                              ║      │ │  • Immortal Decay (6mo)  │ │      ║
                              ║      │ │  • Resonance Decay (7d)  │ │      ║
                              ║      │ │  • Moderation (candle)   │ │      ║
                              ║      │ └───────────┬──────────────┘ │      ║
                              ║      │             │                │      ║
                              ║      │ ┌───────────▼──────────────┐ │      ║
                              ║      │ │  Presence & Traces Mgr   │ │      ║
                              ║      │ │  (Pulsing Now aura)      │ │      ║
                              ║      │ └───────────┬──────────────┘ │      ║
                              ║      │             │                │      ║
                              ║      │ ┌───────────▼──────────────┐ │      ║
                              ║      │ │   RTB Auction Engine     │ │      ║
                              ║      │ │   (<50ms, atomic split)  │ │      ║
                              ║      │ └───────────┬──────────────┘ │      ║
                              ║      └─────────────┼────────────────┘      ║
                              ║                    │                       ║
                              ║      ┌─────────────┼────────────────┐      ║
                              ║      │             │                │      ║
                              ║      ▼             ▼                ▼      ║
                              ║  ┌────────┐   ┌─────────┐    ┌──────────┐  ║
                              ║  │  PG 16 │   │ Redis 7 │    │  candle  │  ║
                              ║  │ (vault)│   │ (live)  │    │  (AI)    │  ║
                              ║  └────────┘   └─────────┘    └──────────┘  ║
                              ║                                                       ║
                              ╚═══════════════ resonance-net (Docker) ═══════════════╝
```

## Flow 1 — The Blind Vault Onboarding

```
┌────────────┐                       ┌────────────┐                  ┌──────────┐
│  Browser   │                       │   Axum     │                  │ Postgres │
└──────┬─────┘                       └─────┬──────┘                  └────┬─────┘
       │                                   │                              │
   1.  │ user types (username, email, pw)  │                              │
   2.  │ Web Crypto:                       │                              │
   │   │   - derive Ed25519 keypair        │                              │
   │   │   - AES-GCM encrypt(email,        │                              │
   │   │     key=HKDF(pw))                 │                              │
   │   │   - blind_index = HMAC-SHA256(    │                              │
   │   │     BLIND_INDEX_KEY, email)       │                              │
   │   │     [truncated to 96 bits]        │                              │
   │   │                                   │                              │
   3.  │ Web Worker solves PoW puzzle:     │                              │
   │   │   find nonce where SHA256(        │                              │
   │   │     username || challenge ||      │                              │
   │   │     nonce) has N leading zeros    │                              │
   │   │                                   │                              │
   4.  │ POST /register {                  │                              │
   │   │   username,                       │                              │
   │   │   email_ciphertext,               │                              │
   │   │   email_blind_index,              │                              │
   │   │   public_key,                     │                              │
   │   │   pow: {challenge, nonce}         │                              │
   │   │ }                                 │                              │
   │   │ ─────────────────────────────────►│                              │
   │   │                                   │ 5. verify PoW (cheap)        │
   │   │                                   │ 6. verify blind index unique │
   │   │                                   │ ────────────────────────────►│
   │   │                                   │   INSERT users (...)         │
   │   │                                   │   RETURNING id               │
   │   │                                   │ ◄────────────────────────────│
   │   │                                   │ 7. zeroize() the OTP buffer  │
   │   │                                   │ 8. enqueue Blind Email Relay │
   │   │                                   │    (send OTP via 3rd-party   │
   │   │                                   │     relay that never sees    │
   │   │                                   │     the cleartext email)     │
   │   │ ◄─────────────────────────────────│                              │
   │   │  { user_id, otp_challenge_id }    │                              │
   │   │                                   │                              │
   9.  │ user enters OTP (received via     │                              │
   │   │ blind relay). POST /verify-otp    │                              │
   │   │ ─────────────────────────────────►│                              │
   │   │                                   │ 10. verify OTP, zeroize(),   │
   │   │                                   │     mark email_verified=true │
   │   │ ◄─────────────────────────────────│                              │
   │   │  { session_token }                │                              │
       │                                   │                              │
       │ Daily interaction:                │                              │
   11. │ open Ed25519 keypair locally      │                              │
   12. │ sign every request body+timestamp │                              │
   │   │ ─────────────────────────────────►│                              │
   │   │                                   │ 13. Signature MW verifies    │
   │   │                                   │     Ed25519 sig vs pubkey    │
```

## Flow 2 — The Lifecycle Cron

```
            ┌─────────────────────────────────────────────┐
            │      tokio-cron-scheduler (UTC)             │
            └─────────────┬───────────────────────────────┘
                          │
       ┌──────────────────┼──────────────────┐
       ▼                  ▼                  ▼
   Evaporate          Immortal           Resonance
   (03:00 daily)      Decay              Decay
                      (Sun 04:00)        (every 15 min)
       │                  │                  │
       ▼                  ▼                  ▼
  pulses WHERE       pulses WHERE       connections WHERE
  age > 30 days      is_preserved       last_interaction_at
  AND NOT            = true AND         < now() - 7 days
  is_preserved       last_interaction   ────────────────────
  ─────────────      _at < now()-180d   resonance_score *= 0.5
  destroy_key(       ─────────────      if resonance_score < 5
    encryption_      delete pulse       → DELETE connection
    key_id)          (immortal revocation)
  DELETE pulse
```

## Flow 3 — RTB Auction (<50ms)

```
   Browser opens feed slot
        │
        ▼
   POST /rtb/bid  { slot_id, user_context_blinded }
        │
        ▼
   ┌─────────────────────────────────────┐
   │   RTB Engine (Axum + tokio::time)   │
   │                                     │
   │   1. fan-out bid requests to        │
   │      N DSPs (Pub/Sub over Redis)    │
   │   2. collect first K bids within    │
   │      45ms (RTB_AUCTION_TIMEOUT_MS)  │
   │   3. second-price auction           │
   │   4. atomic revenue split:          │
   │      creator_balance += bid*0.70    │
   │      platform_balance += bid*0.30   │
   │      (Postgres UPDATE ... RETURNING)│
   │   5. return creative payload        │
   └─────────────┬───────────────────────┘
                 │
                 ▼
        total latency budget <50ms
```

## Flow 4 — Presence & Traces (Pulsing Now)

```
   User A opens profile of User B
        │
        ▼
   SVID=signed timestamp
   POST /presence/pulse  { target=B, sig=A }
        │
        ▼
   ┌─────────────────────────────────────┐
   │  Presence Manager                   │
   │                                     │
   │  1. verify Ed25519 sig              │
   │  2. check resonance(A,B) > 0.5      │
   │     (yes) → SADD presence:B A       │
   │            with TTL 60s             │
   │            → broadcast "Pulsing     │
   │              Now" via WS to B       │
   │     (no)  → silent                  │
   │  3. INSERT into traces (...)        │
   │     with expires_at = now()+7d      │
   └─────────────────────────────────────┘

   The "Pulsing Now" golden aura only renders for pairs whose
   resonance_score is above 50%. Strangers never see presence.
```

## Flow 5 — Live Gravity Feed (Nile Flow)

```
   User A publishes a "نبضة" (pulse)
        │
        ▼
   POST /pulses  { ciphertext, sig, ephemeral_pubkey }
        │
        ▼
   ┌─────────────────────────────────────┐
   │  Pulse Handler                      │
   │                                     │
   │  1. verify sig                      │
   │  2. AI moderation (candle, async)   │
   │     toxicity > 0.7 → cooling period │
   │  3. INSERT pulses (...)             │
   │  4. ZADD feed:glow <now()> pulse_id │
   │  5. PUBLISH feed:new pulse_id       │
   │     (Redis Pub/Sub → all WS clients)│
   └─────────────────────────────────────┘
        │
        ▼
   Each connected WS client receives the pulse_id,
   fetches the full ciphertext, decrypts client-side
   with the ephemeral key, and Svelte transitions
   the new card in with "Nile Flow" easing.
```
