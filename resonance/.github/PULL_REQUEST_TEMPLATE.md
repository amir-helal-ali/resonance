<!-- Thank you for opening a PR! Please fill in the sections below. -->

## Summary
<!-- One or two sentences describing what this PR changes. -->

## Motivation
<!-- Why is this change needed? Link issues with `Fixes #123` or `Refs #123`. -->

## Changes
<!-- Bullet list of concrete changes. -->
-
-
-

## Type of Change
- [ ] 🐛 Bug fix (non-breaking)
- [ ] ✨ New feature (non-breaking)
- [ ] 💥 Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] 📚 Documentation only
- [ ] 🧪 Test only
- [ ] 🔒 Security fix (link to security advisory if any)
- [ ] ♻️ Refactor (no functional change)

## Affected Components
- [ ] Backend (Rust)
- [ ] Frontend (SvelteKit)
- [ ] Database migration (NEW migration file added)
- [ ] Blind Email Relay
- [ ] Docker / Infra
- [ ] CI / CD

## Checklist
- [ ] `cargo fmt --all` passes
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test --all-features` passes
- [ ] If a new SQL query was added, I ran `make sqlx-prepare` and committed `.sqlx/`
- [ ] If a new env var was added, I added it to `.env.example` and `docker-compose.yml`
- [ ] If a new endpoint was added, I documented it in `ARCHITECTURE.md`
- [ ] I have NOT committed any secrets, OTPs, real user data, or `.env` files
- [ ] I have NOT committed any copyrighted material I don't have the rights to

## Migration Notes
<!-- If this PR adds a DB migration, describe how operators should apply it.
     If it changes the cryptographic protocol, describe the upgrade path. -->

## Screenshots / Recordings
<!-- For UI changes, attach before/after screenshots or a short screen recording. -->

## Reviewer Notes
<!-- Anything specific reviewers should focus on? Tricky edge cases? -->
