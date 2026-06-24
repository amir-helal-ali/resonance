# ============================================================================
# Resonance Backend — Multi-Stage Dockerfile
# Stage 1: chef  — cargo-chef planner (caches dependency graph)
# Stage 2: builder — compiles dependencies then app (max layer reuse)
# Stage 3: runtime — debian:bookworm-slim, non-root, no toolchain
# ============================================================================
# This produces a ~45MB image for a fully-featured Axum + candle + SQLx backend.

# ---------- Stage 1: chef (plan) ----------
FROM rust:1.83-slim AS chef
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked --version ^0.1
WORKDIR /app

# ---------- Stage 2: builder (compile) ----------
FROM chef AS builder
# Build dependencies for the candle AI crate (CPU backend).
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential pkg-config libssl-dev \
    && rm -rf /var/lib/lists/*

WORKDIR /app

# 2a. Plan the recipe from Cargo manifests (no source needed yet).
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
RUN cargo chef prepare --recipe-path recipe.json

# 2b. Build dependencies only — this layer is cached across source changes.
RUN cargo chef cook --release --recipe-path recipe.json

# 2c. Compile the actual application. Build BOTH binaries.
COPY . .
# SQLx offline mode: use pre-recorded query metadata (sqlx prepare).
ENV SQLX_OFFLINE=true
ENV OPENSSL_NO_VENDOR=0
RUN cargo build --release --bins

# ---------- Stage 3: runtime (run) ----------
FROM debian:bookworm-slim AS runtime
# Minimal runtime: just the SSL libs + CA certs.
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 resonance \
    && useradd  --system --uid 10001 --gid resonance --no-create-home --shell /usr/sbin/nologin resonance

# Drop-privileges: run as `resonance`, never root.
USER resonance
WORKDIR /app

# Copy the compiled binaries and the SQL migrations (for runtime verification).
COPY --from=builder --chown=resonance:resonance /app/target/release/resonance-backend /app/resonance-backend
COPY --from=builder --chown=resonance:resonance /app/target/release/blind-email-relay /app/blind-email-relay
COPY --from=builder --chown=resonance:resonance /app/migrations /app/migrations

# tini = proper PID 1 signal handling (so Tokio shuts down cleanly on SIGTERM).
ENTRYPOINT ["/usr/bin/tini", "--"]
EXPOSE 8080

# Tokio runtime flags for predictable latency on multi-core hosts:
#   RUST_LOG — log level
#   TOKIO_WORKER_THREADS — defaults to CPU count
ENV RUST_LOG="resonance=info,axum=info,tower_http=info"
CMD ["/app/resonance-backend"]
