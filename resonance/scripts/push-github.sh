#!/usr/bin/env bash
# ============================================================================
# scripts/push-github.sh — صدى (Resonance)
# One-shot script: initializes a Git repo, creates structured commits, creates
# a GitHub repository, and pushes everything to the main branch.
#
# Usage:
#   GH_TOKEN=ghp_xxx ./scripts/push-github.sh [repo-name] [visibility]
#
# Arguments:
#   repo-name   : (optional) GitHub repo name. Default: "resonance"
#   visibility  : (optional) "public" or "private". Default: "public"
#
# Environment:
#   GH_TOKEN    : (required) GitHub Personal Access Token with `repo` scope.
#   GH_USER     : (optional) GitHub username. Auto-detected from the token if
#                 not set.
#   GH_EMAIL    : (optional) Git author email. Default: "<user>@users.noreply.github.com"
# ============================================================================

set -euo pipefail

# ---- Argument validation ----
REPO_NAME="${1:-resonance}"
VISIBILITY="${2:-public}"

if [[ -z "${GH_TOKEN:-}" ]]; then
  echo "❌ GH_TOKEN is not set. Export it first:"
  echo "   export GH_TOKEN=ghp_xxxxxxxxxxxx"
  exit 1
fi

if ! command -v git >/dev/null 2>&1; then
  echo "❌ git is not installed."
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "❌ curl is not installed."
  exit 1
fi

# ---- Detect GitHub user from token ----
echo "🔐 Verifying GitHub token..."
GH_API_USER_RESP=$(curl -sS -H "Authorization: token ${GH_TOKEN}" \
  -H "Accept: application/vnd.github+json" \
  https://api.github.com/user)

GH_USER="${GH_USER:-$(echo "${GH_API_USER_RESP}" | grep -o '"login": *"[^"]*"' | head -1 | sed 's/.*"login": *"\([^"]*\)".*/\1/')}"

if [[ -z "${GH_USER}" || "${GH_USER}" == "null" ]]; then
  echo "❌ Could not detect GitHub username. The token may be invalid."
  echo "   Response: ${GH_API_USER_RESP}"
  exit 1
fi

GH_EMAIL="${GH_EMAIL:-${GH_USER}@users.noreply.github.com}"
echo "✅ Token valid. Detected user: ${GH_USER}"

# ---- Configure Git ----
cd "$(dirname "$0")/.."   # cd to the repo root (resonance/)

git init -b main
git config user.name  "${GH_USER}"
git config user.email "${GH_EMAIL}"

# Safety: never commit secrets
if [[ -f .env && -z "${ALLOW_ENV_COMMIT:-}" ]]; then
  echo "⚠️  .env detected — moving it aside to avoid accidental commit."
  mv .env ".env.backup.$(date +%s)"
fi

# Verify .gitignore exists and covers secrets
if ! grep -q "^\.env$" .gitignore 2>/dev/null; then
  echo ".env" >> .gitignore
fi
if ! grep -q "secrets/" .gitignore 2>/dev/null; then
  echo "secrets/" >> .gitignore
fi

# ---- Commit 1: project skeleton + Docker infra ----
echo ""
echo "📦 Commit 1/6: infra (docker, dockerfiles, makefile, gitignore)"
git add docker-compose.yml docker-compose.dev.yml Makefile .gitignore \
        backend/Dockerfile backend/.dockerignore \
        frontend/Dockerfile frontend/.dockerignore \
        .env.example backend/.env.example frontend/.env.example
git commit -q -m "chore(infra): docker-compose for 5 services + multi-stage Dockerfiles + Makefile

- docker-compose.yml: db (PG16), redis (7), backend (Rust), frontend (SvelteKit), relay
- docker-compose.dev.yml: hot-reload override (cargo-watch + vite HMR)
- backend/Dockerfile: multi-stage (chef → builder → runtime), non-root, tini as PID 1
- frontend/Dockerfile: multi-stage (deps → builder → runtime)
- Makefile: 15 convenience targets
- .dockerignore files to keep build context small"

# ---- Commit 2: database schema ----
echo "📦 Commit 2/6: database schema"
git add backend/migrations/
git commit -q -m "feat(db): PostgreSQL schema — blind vault, pulses, connections, traces, goals, jury

- 0001_init.sql: users (email_ciphertext, email_blind_index, public_key, imprint, horizon),
  pulses (lifecycle, is_preserved, last_interaction_at, encryption_key_id),
  connections (resonance_score), traces (7d TTL), moderation_queue, RLS enabled
- 0002_goals_jury_sessions.sql: goals, goal_candles, jury_panels, pulse_reports, sessions
- touch_updated_at trigger for honest timestamps
- Unique constraint on email_blind_index prevents duplicate registration without revealing email"

# ---- Commit 3: Rust backend core (crypto, db, errors, middleware) ----
echo "📦 Commit 3/6: backend core (crypto, db, errors, middleware, state)"
git add backend/Cargo.toml backend/src/lib.rs backend/src/main.rs \
        backend/src/state.rs backend/src/errors/ backend/src/crypto/ \
        backend/src/db/ backend/src/middleware/ backend/src/cron/
git commit -q -m "feat(backend): core crypto, DB layer, Ed25519 signature middleware, cron scheduler

- crypto/blind_vault.rs: verify_pow (configurable difficulty), compute_blind_index
  (HMAC-SHA256 → 96 bits), verify_ed25519_signature, ZeroizingOtp
- db/: SQLx pool, typed row models, queries for users/pulses/connections/traces/moderation
- errors/mod.rs: thiserror AppError → IntoResponse (privacy-safe JSON bodies)
- middleware/signature.rs: Ed25519 signature verification + replay protection (±60s skew)
- cron/mod.rs: 7 tokio-cron-scheduler jobs (evaporate, immortal decay, resonance
  decay, prune traces, moderation cooling + jury summon, jury expiry)
- main.rs: Axum router, graceful shutdown, logging (tracing-subscriber JSON)"

# ---- Commit 4: Rust backend handlers ----
echo "📦 Commit 4/6: backend handlers + blind email relay binary"
git add backend/src/handlers/ backend/src/presence/ backend/src/bin/
git commit -q -m "feat(backend): handlers for vault, pulses, connections, goals, jury, RTB, presence, observability

- handlers/vault.rs: /register (PoW + blind index dedup), /verify-otp, /pow/challenge
- handlers/blind_index.rs: POST /blind-index with Zeroizing guard + rate-limit fingerprint
- handlers/pulses.rs: POST /pulses (ciphertext only), GET /feed/glow, WS /ws
- handlers/interactions.rs: echo / save / comment / report (3 reports → jury)
- handlers/connections.rs: sync, list, Jaccard suggestions, unsync
- handlers/goals.rs: create, list, light candle (atomic)
- handlers/jury.rs: Transient Jury (5 random jurors, quorum 3, 24h)
- handlers/rtb.rs: Vickrey second-price auction + atomic rev-share
- handlers/moderation.rs: candle AI (lazy-loaded BERT-tiny) + Thermodynamic Cooling
  + HeuristicToxicityModel fallback
- handlers/observability.rs: /health, /ready, /metrics (Prometheus)
- presence/mod.rs: Pulsing Now aura + Passing Traces (7d TTL)
- bin/blind_email_relay.rs: standalone OTP relay with ZeroizingString"

# ---- Commit 5: Rust tests ----
echo "📦 Commit 5/6: rust integration tests"
git add backend/tests/
git commit -q -m "test(backend): crypto + API smoke tests

- tests/crypto_spec.rs: PoW round-trip, blind index determinism, Ed25519
  signature round-trip, tampered body rejection, ZeroizingOtp verify + drop
- tests/api_smoke.rs: /health 200, unknown routes 404, PoW env reading"

# ---- Commit 6: frontend ----
echo "📦 Commit 6/6: SvelteKit frontend"
git add frontend/
git commit -q -m "feat(frontend): SvelteKit + TailwindCSS + Web Crypto + Web Workers

- crypto/blind_vault.ts: AES-GCM email encryption, Ed25519 keypair + signing,
  IndexedDB storage for private key (NEVER localStorage)
- workers/pow.worker.ts: off-main-thread PoW solver using @noble/hashes
- api/: typed signed-fetch wrapper for all backend endpoints
- components/:
  - RegisterForm (Blind Vault onboarding: encrypt → blind index → PoW → register → OTP)
  - Composer (per-pulse AES-GCM key, encryption local)
  - LivingProfile (بصمتي + أُفقي in Amiri, شموع الدعم, Nile Presence Bar)
  - SunCycleAura (lifecycle color ring: شروق/توهج/غروب/حذف)
  - NilePresenceBar (Pulsing Now indicator)
  - LiveFeed (WebSocket + Nile Flow transitions + Odometer counters)
  - Odometer (animated speedometer-style counter)
  - Navbar
- routes/: register, feed (+ Composer), profile, connections, traces, jury
- tailwind.config.js: papyrus / astral-blue / egyptian-gold palette
- app.css: Sun Cycle Aura + Nile Flow + aura-pulse keyframes"

# ---- Commit 7: docs + GitHub config ----
echo "📦 Commit 7/7: docs, CI, GitHub config"
git add README.md ARCHITECTURE.md CONTRIBUTING.md SECURITY.md LICENSE \
        .github/ scripts/
git commit -q -m "docs: README with badges, architecture diagrams, contributing guide, security policy

- README.md: badges, quick start, API surface table, roadmap, community links
- ARCHITECTURE.md: textual flow diagrams for Blind Vault, Lifecycle, RTB, Presence, Live Feed
- CONTRIBUTING.md: coding standards (Rust + Svelte + SQL), PR process, areas needing help
- SECURITY.md: threat model, crypto bill of materials, disclosure policy
- LICENSE: MIT
- .github/workflows/ci.yml: 5 jobs (rust-lint, rust-test, rust-build, frontend, docker-build)
- .github/ISSUE_TEMPLATE: bug_report, feature_request, security_vulnerability
- .github/PULL_REQUEST_TEMPLATE.md, CODEOWNERS, FUNDING.yml, dependabot.yml, CODE_OF_CONDUCT.md
- scripts/push-github.sh: this script"

# ---- Create GitHub repo ----
echo ""
echo "🌐 Creating GitHub repo: ${GH_USER}/${REPO_NAME} (${VISIBILITY})..."

VISIBILITY_PAYLOAD=$([[ "${VISIBILITY}" == "private" ]] && echo "true" || echo "false")

CREATE_RESP=$(curl -sS -X POST \
  -H "Authorization: token ${GH_TOKEN}" \
  -H "Accept: application/vnd.github+json" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  https://api.github.com/user/repos \
  -d "{\"name\":\"${REPO_NAME}\",\"private\":${VISIBILITY_PAYLOAD},\"description\":\"صدى — a chrono-social, privacy-first platform. 100% Rust + SvelteKit.\"}")

CLONE_URL=$(echo "${CREATE_RESP}" | grep -o '"clone_url": *"[^"]*"' | head -1 | sed 's/.*"clone_url": *"\([^"]*\)".*/\1/')

if [[ -z "${CLONE_URL}" ]]; then
  echo "❌ Failed to create repo. Response:"
  echo "${CREATE_RESP}"
  exit 1
fi

# Use HTTPS with token embedded so push works non-interactively.
PUSH_URL="https://${GH_USER}:${GH_TOKEN}@github.com/${GH_USER}/${REPO_NAME}.git"

echo "✅ Repo created: ${CLONE_URL}"

# ---- Add remote + push ----
git remote remove origin 2>/dev/null || true
git remote add origin "${PUSH_URL}"

echo ""
echo "🚀 Pushing to origin/main..."
git push -u origin main

echo ""
echo "🎉 Done! Your project is live at:"
echo "   https://github.com/${GH_USER}/${REPO_NAME}"
echo ""
echo "Next steps:"
echo "  1. Update the badge URLs in README.md to point to your actual repo"
echo "  2. Configure branch protection: Settings → Branches → Add rule for 'main'"
echo "  3. Enable GitHub Discussions: Settings → Features → Discussions"
echo "  4. Add the GH_TOKEN as a CI secret: Settings → Secrets → Actions → New"
echo "  5. Replace placeholder emails (security@resonance.local, conduct@resonance.local)"
echo "     with real addresses"
