# ============================================================================
# Resonance (صدى) — Makefile
# Convenience commands for development.
# ============================================================================

.PHONY: help up down logs build rebuild test lint fmt migrate sqlx-prepare \
        frontend-install frontend-dev frontend-build relay-dev shell-db shell-redis

# Default target: show available commands.
help:
	@echo "صدى (Resonance) — available commands:"
	@echo ""
	@echo "  make up              — start all services (db, redis, backend, frontend, relay)"
	@echo "  make down            — stop all services"
	@echo "  make logs            — tail logs from all services"
	@echo "  make build           — rebuild all images"
	@echo "  make rebuild         — rebuild without cache"
	@echo "  make test            — run Rust tests"
	@echo "  make lint            — clippy + fmt check (Rust)"
	@echo "  make fmt             — apply rustfmt"
	@echo "  make migrate         — apply DB migrations"
	@echo "  make sqlx-prepare    — regenerate .sqlx/ offline metadata"
	@echo "  make frontend-install— pnpm install"
	@echo "  make frontend-dev    — vite dev server on :3000"
	@echo "  make frontend-build  — vite build"
	@echo "  make relay-dev       — run blind-email-relay locally"
	@echo "  make shell-db        — psql into the DB container"
	@echo "  make shell-redis     — redis-cli into Redis"

# ----------------------------------------------------------------------------
# Docker orchestration
# ----------------------------------------------------------------------------
up:
	docker compose up -d --build

down:
	docker compose down

logs:
	docker compose logs -f --tail=200

build:
	docker compose build

rebuild:
	docker compose build --no-cache

# ----------------------------------------------------------------------------
# Rust development
# ----------------------------------------------------------------------------
test:
	cd backend && cargo test --all-features --no-fail-fast

lint:
	cd backend && cargo clippy --all-targets -- -D warnings
	cd backend && cargo fmt --all -- --check

fmt:
	cd backend && cargo fmt --all

migrate:
	cd backend && sqlx migrate run --source migrations --database-url $$DATABASE_URL

# Regenerate the .sqlx/ offline metadata. Run after changing any query.
sqlx-prepare:
	cd backend && cargo sqlx prepare -- --lib

# ----------------------------------------------------------------------------
# Frontend development
# ----------------------------------------------------------------------------
frontend-install:
	cd frontend && pnpm install

frontend-dev:
	cd frontend && pnpm dev

frontend-build:
	cd frontend && pnpm build

# ----------------------------------------------------------------------------
# Blind Email Relay
# ----------------------------------------------------------------------------
relay-dev:
	cd backend && cargo run --bin blind-email-relay

# ----------------------------------------------------------------------------
# Shell into services
# ----------------------------------------------------------------------------
shell-db:
	docker compose exec db psql -U resonance -d resonance

shell-redis:
	docker compose exec redis redis-cli
