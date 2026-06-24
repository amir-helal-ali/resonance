# Contributing to صدى (Resonance)

شكرًا لاهتمامك بالمساهمة في صدى! هذا الدليل يشرح كيف تبدأ.

## Code of Conduct

كن محترمًا. النقد البنّاء مرحب به؛ الهجوم الشخصي مرفوض. نحن نبني منصة
للناس — فلنعامل بعضنا كما نريد أن تُعامل مجتمعاتنا.

## Getting Started

```bash
git clone https://github.com/<your-org>/resonance.git
cd resonance
cp .env.example .env          # adjust secrets
make up                       # start all services
make logs                     # tail logs
```

For development with hot-reload:

```bash
docker compose -f docker-compose.yml -f docker-compose.dev.yml up
```

## Architecture Quick Reference

- **Backend**: 100% Rust (Axum + Tokio + SQLx + candle + zeroize). See `ARCHITECTURE.md`.
- **Frontend**: SvelteKit + TailwindCSS + Web Crypto + Web Workers.
- **Database**: PostgreSQL 16 (persistent vault) + Redis 7 (live state).
- **Blind Email Relay**: standalone Rust binary consuming Redis jobs.

Read `ARCHITECTURE.md` for the full data flow (Blind Vault onboarding,
Lifecycle cron, RTB auction, Presence, Live Feed).

## Coding Standards

### Rust
- `cargo fmt --all` — formatting is enforced in CI.
- `cargo clippy --all-targets -- -D warnings` — clippy is enforced in CI.
- Every `pub fn` must have a doc comment.
- Every fallible operation returns `AppResult<T>` (see `src/errors/mod.rs`).
- Secrets in memory must be wrapped in `Zeroizing<...>`.
- Never log OTPs, blind indexes, ciphertexts, or signatures.

### SvelteKit
- Use TypeScript strictly (no `any`).
- All API calls go through `src/lib/api/*.ts` (signed fetch wrapper).
- Tailwind classes only — no inline `style=""` except for animations.
- Arabic copy lives in components (not in stores) so translators can find it.

### SQL
- Every migration is `0001_*.sql`, `0002_*.sql`, ... — monotonic, append-only.
- Never edit a merged migration. Write a new one.
- Every `CREATE TABLE` includes `created_at` and `updated_at` (with the
  `touch_updated_at` trigger where applicable).
- Every foreign key declares `ON DELETE` explicitly.

## Pull Request Process

1. **Branch**: `feat/short-description` or `fix/short-description`.
2. **Commit messages**: conventional commits
   (`feat:`, `fix:`, `chore:`, `docs:`, `refactor:`, `test:`).
3. **Tests**: every PR that changes business logic must include or update
   tests under `backend/tests/` or `frontend/test/`.
4. **Migration changes**: must include the `UP` SQL; if a destructive change
   is unavoidable, include a rollback script under `migrations/down/`.
5. **Review**: at least one maintainer approval required.
6. **CI**: all 5 jobs must pass (`rust-lint`, `rust-test`, `rust-build`,
   `frontend`, `docker-build`).

## Reporting Security Vulnerabilities

**DO NOT open a public GitHub issue.** Email `security@resonance.local` with:
- a description of the vulnerability,
- the affected component (Blind Vault, RTB, etc.),
- a minimal reproduction (if possible).

You will receive an acknowledgement within 48 hours and a fix timeline
within 7 days. See `SECURITY.md` for the full policy.

## Areas That Need Help

- 🌐 **i18n**: the UI is Egyptian Arabic only — we want MSA, English, French.
- 🧠 **candle models**: replace the heuristic toxicity model with a fine-tuned BERT.
- 🛡️ **TEE integration**: move the Blind Email Relay into an SGX/SEV enclave.
- 📊 **Analytics**: a Grafana dashboard consuming the `/metrics` endpoint.
- 🧪 **E2E tests**: Playwright suite for the registration → pulse → echo flow.

## License

By contributing, you agree that your contributions will be licensed under the
MIT License (see `LICENSE`).
