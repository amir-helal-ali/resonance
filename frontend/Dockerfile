# ============================================================================
# Resonance Frontend — SvelteKit Multi-Stage Dockerfile
# Stage 1: deps     — install pnpm + node_modules (cached)
# Stage 2: builder  — build SvelteKit with adapter-node
# Stage 3: runtime  — node:20-slim, non-root, only build output + node_modules
# ============================================================================

# ---------- Stage 1: deps ----------
FROM node:20-bookworm-slim AS deps
WORKDIR /app
RUN corepack enable && corepack prepare pnpm@9.15.0 --activate
COPY package.json pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile

# ---------- Stage 2: builder ----------
FROM node:20-bookworm-slim AS builder
WORKDIR /app
RUN corepack enable && corepack prepare pnpm@9.15.0 --activate
COPY --from=deps /app/node_modules ./node_modules
COPY . .
ENV NODE_ENV=production
# SvelteKit adapter-node emits ./build
RUN pnpm run build

# ---------- Stage 3: runtime ----------
FROM node:20-bookworm-slim AS runtime
WORKDIR /app
ENV NODE_ENV=production
RUN groupadd --system --gid 10001 resonance \
 && useradd  --system --uid 10001 --gid resonance --no-create-home --shell /usr/sbin/nologin resonance

# adapter-node needs only ./build and the production node_modules.
COPY --from=builder --chown=resonance:resonance /app/build ./build
COPY --from=builder --chown=resonance:resonance /app/node_modules ./node_modules
COPY --from=builder --chown=resonance:resonance /app/package.json ./package.json

USER resonance
EXPOSE 3000
CMD ["node", "build"]
