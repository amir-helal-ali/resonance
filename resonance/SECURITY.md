# Security Policy — صدى (Resonance)

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| main    | ✅                 |
| < 1.0   | ❌ (pre-release)   |

## Threat Model (Summary)

صدى is designed against the following adversaries:

1. **Curious operator** — the server-side team has full DB access but should
   NOT be able to read user emails, pulse contents, or DM bodies.
   - Mitigation: emails encrypted client-side (AES-GCM under HKDF(password)).
     Pulse bodies encrypted with per-pulse AES-GCM keys wrapped under the
     user's KEK. Ed25519 private keys never leave the browser.

2. **Compromised database** — an attacker obtains a Postgres dump.
   - Mitigation: only ciphertext + HMAC blind indexes are stored. Without
     the per-user KEK (derived from the password) the data is unrecoverable.
     The HMAC key (`BLIND_INDEX_KEY`) is held in server env, not the DB.

3. **Bot registration** — automated account creation.
   - Mitigation: Proof-of-Work puzzle (configurable difficulty, default 20
     leading zero bits ≈ 1M SHA-256 ops).

4. **Replay attack** — a captured signed request is re-sent.
   - Mitigation: every signed request carries a unix-millis timestamp;
     the middleware rejects any with skew > ±60s.

5. **Toxic content** — coordinated harassment or spam.
   - Mitigation: AI moderation (`candle`) + Thermodynamic Cooling +
     Transient Jury (5 random users, 24h, quorum 3).

## Reporting a Vulnerability

**DO NOT open a public GitHub issue.** Instead:

1. Email **security@resonance.local** with:
   - a clear description of the vulnerability,
   - the affected component (Blind Vault, RTB, Moderation, etc.),
   - a minimal reproduction (PoC code, screenshots, or step-by-step),
   - your preferred contact channel.

2. You will receive an acknowledgement within **48 hours**.

3. We will triage within **7 days** and propose a fix timeline:
   - Critical (RCE, auth bypass, key leak) → hotfix within 24h.
   - High (data leak, DoS) → patch within 7 days.
   - Medium / Low → next release.

4. We coordinate disclosure with you before publishing any advisory. We
   credit reporters in the release notes unless they prefer to remain
   anonymous.

## Cryptographic Bill of Materials

| Purpose | Algorithm | Library | Notes |
|---------|-----------|---------|-------|
| User signing | Ed25519 | `ed25519-dalek`, `@noble/ed25519` | keypair generated in browser |
| Email encryption | AES-GCM-256 | Web Crypto API | KEK = HKDF-SHA256(password, salt) |
| Blind index | HMAC-SHA256 truncated to 96 bits | `hmac` (Rust), Web Crypto | key = `BLIND_INDEX_KEY` env |
| Pulse encryption | AES-GCM-256 | Web Crypto API | per-pulse key, wrapped under KEK |
| PoW puzzle | SHA-256 | `sha2`, `@noble/hashes` | configurable difficulty |
| OTP storage | n/a (Redis TTL) | Redis | wrapped in `Zeroizing<String>` |
| Request integrity | Ed25519 over `method\npath\ntimestamp\nsha256(body)` | `ed25519-dalek` | skew window ±60s |

## Operational Hygiene

- `BLIND_INDEX_KEY` is 32 bytes, base64-encoded, loaded from env at startup.
  Rotate via secret manager; rotation invalidates all blind indexes (forces
  re-registration).
- `POSTGRES_PASSWORD` must be a 32+ char random string.
- Redis runs with `rename-command FLUSHALL ""` etc. (see `docker-compose.yml`).
- The `resonance` container user is non-root (uid 10001).
- Postgres logs are disabled (`log_statement=none`) so OTPs in queries are
  never captured.
- The Blind Email Relay is a separate container so it can be placed behind
  a tighter network policy (no inbound ports).

## Known Limitations (Pre-1.0)

- The `candle` toxicity model is a heuristic placeholder until weights are
  downloaded to `/app/models/toxicity/`.
- The Blind Email Relay does NOT yet decrypt the email (would require TEE
  integration). For dev it logs the dispatch without sending.
- WebSocket connections are not yet rate-limited. Production deployments
  should put `cloudflared` or `nginx` with `limit_req` in front.
- The CORS layer is `very_permissive` — tighten before public launch.
