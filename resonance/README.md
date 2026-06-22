# صدى — Resonance

A chrono-social, privacy-first social platform that rethinks identity, time, and presence.

> Backend: 100% Rust (Axum, Tokio, SQLx, candle, zeroize).
> Frontend: SvelteKit + TailwindCSS + Web Crypto + Web Workers.

## Quick Start

```bash
cd resonance
cp .env.example .env       # then edit secrets
make up                    # builds + starts all 5 services
```

- Frontend: http://localhost:3000
- Backend:  http://localhost:8080
- Health:   http://localhost:8080/health

Other useful `make` targets: `make logs`, `make down`, `make test`, `make lint`,
`make sqlx-prepare`, `make shell-db`, `make shell-redis`.

## Project Structure

```
resonance/
├── docker-compose.yml         # 5-service orchestration (db, redis, backend, frontend, relay)
├── Makefile                   # convenience commands
├── ARCHITECTURE.md            # textual architecture + flow diagrams
├── .github/workflows/ci.yml   # fmt + clippy + test + build + docker
├── backend/
│   ├── Dockerfile             # multi-stage (chef → builder → runtime)
│   ├── Cargo.toml
│   ├── migrations/
│   │   ├── 0001_init.sql          # users, pulses, connections, traces, ...
│   │   └── 0002_goals_jury_sessions.sql  # goals, jury_panels, sessions
│   ├── src/
│   │   ├── lib.rs            # library facade (for integration tests)
│   │   ├── main.rs           # Axum router + cron start
│   │   ├── state.rs
│   │   ├── errors/mod.rs     # thiserror AppError → IntoResponse
│   │   ├── crypto/blind_vault.rs  # PoW + HMAC blind index + Ed25519 verify
│   │   ├── db/               # SQLx pool + models + queries
│   │   ├── handlers/
│   │   │   ├── vault.rs          # POST /register, /verify-otp, /pow/challenge
│   │   │   ├── blind_index.rs    # POST /blind-index (server HMAC)
│   │   │   ├── pulses.rs         # POST /pulses, GET /feed/glow, /ws
│   │   │   ├── interactions.rs   # POST /pulses/:id/{echo,save,comment,report}
│   │   │   ├── connections.rs    # POST /connections/sync, Jaccard suggestions
│   │   │   ├── goals.rs          # POST /goals, POST /goals/:id/light
│   │   │   ├── jury.rs           # Transient Jury summon + vote
│   │   │   ├── rtb.rs            # Instant Rev-Share RTB engine
│   │   │   └── moderation.rs     # candle AI + Thermodynamic Cooling
│   │   ├── middleware/signature.rs  # Ed25519 signature verification
│   │   ├── presence/mod.rs    # "Pulsing Now" aura + Passing Traces
│   │   ├── cron/mod.rs        # tokio-cron-scheduler (7 jobs)
│   │   └── bin/blind_email_relay.rs  # standalone OTP relay worker
│   └── tests/
│       ├── crypto_spec.rs     # PoW + HMAC + Ed25519 + ZeroizingOtp
│       └── api_smoke.rs       # routing smoke tests
└── frontend/
    ├── Dockerfile             # multi-stage (deps → builder → runtime)
    ├── tailwind.config.js     # papyrus / astral-blue / egyptian-gold
    └── src/
        ├── app.css            # Sun Cycle Aura + Nile Flow keyframes
        ├── lib/
        │   ├── crypto/blind_vault.ts   # Web Crypto (encrypt, sign, KEK)
        │   ├── workers/pow.worker.ts   # PoW solver (off main thread)
        │   ├── workers/pow.client.ts   # Worker wrapper
        │   ├── api/client.ts           # signed fetch wrapper
        │   ├── api/presence.ts
        │   ├── api/pulses.ts
        │   ├── api/connections.ts
        │   ├── api/goals.ts
        │   ├── api/jury.ts
        │   ├── stores/session.ts
        │   └── components/
        │       ├── Navbar.svelte               # top nav
        │       ├── RegisterForm.svelte          # Blind Vault onboarding UI
        │       ├── Composer.svelte              # new pulse composer
        │       ├── LivingProfile.svelte         # بصمتي + أُفقي + شموع
        │       ├── SunCycleAura.svelte          # lifecycle color ring
        │       ├── NilePresenceBar.svelte       # Pulsing Now indicator
        │       ├── LiveFeed.svelte              # WebSocket feed + Nile Flow
        │       └── Odometer.svelte              # animated counter
        └── routes/
            ├── +layout.svelte
            ├── +page.svelte              # home (live feed if logged in)
            ├── register/+page.svelte
            ├── feed/+page.svelte         # + Composer
            ├── profile/+page.svelte
            ├── connections/+page.svelte  # صدى management
            ├── traces/+page.svelte       # آثار عابرة
            └── jury/+page.svelte         # محكمة عابرة
```

## The Seven Cron Jobs

| Job | Schedule | Purpose |
|---|---|---|
| `promote_glow_to_linger` | hourly | Move pulses older than 48h from `glow` to `linger` |
| `evaporate_30_day` | daily 03:00 | Destroy keys for unpreserved pulses >30 days |
| `immortal_decay` | Sunday 04:00 | Revoke preservation for pulses idle 6 months |
| `resonance_decay` | every 15 min | Halve stale (>7d) resonance; delete <5 |
| `prune_traces` | hourly | Delete traces past their 7-day TTL |
| `moderation_cooling_release` | every 2 min | Release cooled pulses + summon juries for toxicity ≥ 0.9 |
| `jury_panel_expiry` | every 5 min | Expire jury panels past their 24h window |

## Privacy Model

| What | Where | Notes |
|---|---|---|
| Email cleartext | browser only | never sent over the wire |
| Email ciphertext | Postgres | AES-GCM, key derived from password |
| Blind index | Postgres | HMAC-SHA256 truncated to 96 bits |
| Ed25519 private key | IndexedDB | never leaves the browser |
| PoW solution | Redis (60s TTL) | single-use, then deleted |
| OTP | Redis (10-min TTL) | wrapped in `Zeroizing<String>` |

## RTB Revenue Split

```
winning_bid * (1 - platform_share_bps / 10000) → creator_balance
winning_bid * (platform_share_bps / 10000)     → platform_balance
```

Default: 30% platform / 70% creator. Atomic via Postgres transaction.

## License

Proprietary. © صدى.
