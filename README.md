# صدى — Resonance

[![CI](https://github.com/amir-helal-ali/resonance/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/amir-helal-ali/resonance/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-egyptian-gold.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.83+-orange.svg)](https://www.rust-lang.org/)
[![SvelteKit](https://img.shields.io/badge/SvelteKit-2.x-ff3e00.svg)](https://kit.svelte.dev/)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)
[![Security Policy](https://img.shields.io/badge/Security-Policy-blue.svg)](SECURITY.md)
[![Made in Egypt](https://img.shields.io/badge/Made%20in-Egypt-✨-red.svg)](#)

> **صدى** — منصة تواصل اجتماعي زمنية خاصة، تعيد تعريف الهوية والزمن والحضور.
> A chrono-social, privacy-first social platform that rethinks identity, time, and presence.

| | |
|---|---|
| **Backend** | 100% Rust — Axum, Tokio, SQLx, Redis, tokio-cron-scheduler, candle, zeroize, ed25519-dalek |
| **Frontend** | SvelteKit, TailwindCSS, Web Crypto API, Web Workers |
| **Storage**  | PostgreSQL 16 (persistent vault) + Redis 7 (live state) |
| **Infra**    | Docker Compose — 5 isolated services on a private network |
| **Identity** | Egyptian-inspired palette: رمال النيل (papyrus), سقف الأقصر الليلي (astral blue), ذهبي مصري (gold) |
| **Fonts**    | Cairo (UI) + Amiri (manifesto) |

---

## Quick Start

```bash
git clone https://github.com/amir-helal-ali/resonance.git
cd resonance
cp .env.example .env       # then edit secrets
make up                    # builds + starts all 5 services
```

- Frontend: http://localhost:3000
- Backend:  http://localhost:8080
- Health:   http://localhost:8080/health
- Ready:    http://localhost:8080/ready
- Metrics:  http://localhost:8080/metrics

Other useful `make` targets: `make logs`, `make down`, `make test`, `make lint`,
`make sqlx-prepare`, `make shell-db`, `make shell-redis`.

For hot-reload development:

```bash
docker compose -f docker-compose.yml -f docker-compose.dev.yml up
```

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

## API Surface

### Public (no signature)
| Method | Path | Purpose |
|---|---|---|
| GET  | `/health` | Liveness probe |
| GET  | `/ready` | Readiness probe (DB + Redis) |
| GET  | `/metrics` | Prometheus exposition |
| GET  | `/pow/challenge` | Issue a PoW challenge (60s TTL) |
| POST | `/register` | Blind Vault onboarding |
| POST | `/verify-otp` | Verify OTP from blind relay |
| POST | `/blind-index` | Compute HMAC blind index for email |
| GET  | `/feed/glow` | Top 50 live glow pulses |
| GET  | `/ws` | Live Gravity Feed WebSocket |

### Protected (Ed25519 signature required)
| Method | Path | Purpose |
|---|---|---|
| POST   | `/pulses` | Create a new نبضة |
| POST   | `/pulses/:id/echo` | Amplify a pulse |
| POST   | `/pulses/:id/save` | Bookmark a pulse |
| POST   | `/pulses/:id/comment` | Encrypted comment |
| POST   | `/pulses/:id/report` | Flag for jury (3 reports → summon) |
| POST   | `/connections/sync` | Bump resonance with another user |
| GET    | `/connections` | List my resonances (≥5) |
| GET    | `/connections/suggest` | Co-Resonance suggestions (Jaccard) |
| DELETE | `/connections/:target` | Un-sync from a user |
| POST   | `/presence/pulse` | Visit a profile (sets aura if resonance >50) |
| GET    | `/presence/:user_id` | Who is Pulsing Now on this user? |
| GET    | `/traces` | My recent Passing Traces (7d TTL) |
| POST   | `/goals` | Create a goal (أُفقي) |
| GET    | `/goals/:user_id` | List a user's goals |
| POST   | `/goals/:id/light` | Light a شمعة الدعم |
| GET    | `/jury/summoned` | Panels where I'm a juror |
| POST   | `/jury/:panel_id/vote` | Cast uphold/release vote |
| POST   | `/rtb/auction` | Run a Vickrey auction + atomic rev-share |

## Contributing & Community

- 📜 [Contributing Guide](CONTRIBUTING.md) — coding standards, PR process, areas needing help
- 🔒 [Security Policy](SECURITY.md) — threat model, vulnerability disclosure, crypto bill of materials
- 🏛️ [Code of Conduct](https://github.com/amir-helal-ali/resonance/blob/main/.github/CODE_OF_CONDUCT.md) — be respectful
- 🐛 [Report a Bug](https://github.com/amir-helal-ali/resonance/issues/new?template=bug_report.md)
- ✨ [Request a Feature](https://github.com/amir-helal-ali/resonance/issues/new?template=feature_request.md)
- 💬 [Discussions](https://github.com/amir-helal-ali/resonance/discussions) — questions, ideas, announcements

## Roadmap

- [ ] **i18n** — Modern Standard Arabic, English, French (currently Egyptian Arabic only)
- [ ] **candle models** — replace heuristic toxicity with a fine-tuned BERT
- [ ] **TEE integration** — move Blind Email Relay into SGX/SEV enclave
- [ ] **Grafana dashboard** — consume `/metrics` for live ops visibility
- [ ] **Playwright E2E** — full registration → pulse → echo flow
- [ ] **Mobile** — React Native or PWA shell with offline PoW
- [ ] **Federation** — ActivityPub bridge for cross-instance صدى

## License

[MIT](LICENSE) — © صدى Contributors.

This project is built with love from Egypt 🇪🇬
