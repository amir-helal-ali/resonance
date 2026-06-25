// ============================================================================
// resonance-backend/src/handlers/observability.rs
// Health, readiness, and Prometheus-style metrics endpoints.
//
//   GET /health       — liveness probe (process is up)
//   GET /ready        — readiness probe (DB + Redis reachable)
//   GET /metrics      — Prometheus text exposition format
// ============================================================================

use axum::{extract::State, response::IntoResponse, Json};
use http::StatusCode;
use redis::AsyncCommands;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::{errors::AppResult, state::AppState};

/// Lightweight process metrics. In production you'd switch to `prometheus`
/// or `metrics` crates with a real registry; this minimal version keeps the
/// dependency footprint small.
#[derive(Default)]
pub struct Metrics {
    pub pulses_created:    AtomicU64,
    pub pulses_evaporated: AtomicU64,
    pub echoes:            AtomicU64,
    pub saves:             AtomicU64,
    pub comments:          AtomicU64,
    pub auctions_run:      AtomicU64,
    pub juries_summoned:   AtomicU64,
    pub ws_connections:    AtomicU64,
}

pub type MetricsHandle = Arc<Metrics>;

pub fn metrics_handle() -> MetricsHandle {
    Arc::new(Metrics::default())
}

/// GET /health — liveness. Always 200 if the process is up.
pub async fn health() -> &'static str {
    "صدى alive"
}

/// GET /ready — readiness. Probes DB + Redis.
pub async fn ready(
    State(state): State<AppState>,
) -> AppResult<impl IntoResponse> {
    // 1. DB: a trivial SELECT 1.
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await
        .is_ok();

    // 2. Redis: PING.
    let mut conn = state.redis.clone();
    let redis_ok: bool = redis::cmd("PING")
        .query_async::<String>(&mut conn)
        .await
        .map(|s| s == "PONG")
        .unwrap_or(false);

    let ready = db_ok && redis_ok;
    let status = if ready { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };

    let body = Json(json!({
        "ready": ready,
        "db":    db_ok,
        "redis": redis_ok,
    }));
    Ok((status, body))
}

/// GET /metrics — Prometheus text exposition.
pub async fn metrics(
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Fetch some live gauges from the DB.
    let users_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    let pulses_glow: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pulses WHERE lifecycle = 'glow'"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    let pulses_linger: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pulses WHERE lifecycle = 'linger'"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let m = &state.metrics;
    let body = format!(
        "# HELP resonance_users_total Total registered users.\n\
         # TYPE resonance_users_total gauge\n\
         resonance_users_total {users_count}\n\
         # HELP resonance_pulses_total Pulses by lifecycle phase.\n\
         # TYPE resonance_pulses_total gauge\n\
         resonance_pulses_total{{phase=\"glow\"}} {pulses_glow}\n\
         resonance_pulses_total{{phase=\"linger\"}} {pulses_linger}\n\
         # HELP resonance_pulses_created_total Total pulses created since startup.\n\
         # TYPE resonance_pulses_created_total counter\n\
         resonance_pulses_created_total {created}\n\
         # HELP resonance_pulses_evaporated_total Total pulses evaporated since startup.\n\
         # TYPE resonance_pulses_evaporated_total counter\n\
         resonance_pulses_evaporated_total {evaporated}\n\
         # HELP resonance_interactions_total Interactions by kind.\n\
         # TYPE resonance_interactions_total counter\n\
         resonance_interactions_total{{kind=\"echo\"}} {echoes}\n\
         resonance_interactions_total{{kind=\"save\"}} {saves}\n\
         resonance_interactions_total{{kind=\"comment\"}} {comments}\n\
         # HELP resonance_auctions_total RTB auctions run.\n\
         # TYPE resonance_auctions_total counter\n\
         resonance_auctions_total {auctions}\n\
         # HELP resonance_juries_summoned_total Jury panels summoned.\n\
         # TYPE resonance_juries_summoned_total counter\n\
         resonance_juries_summoned_total {juries}\n\
         # HELP resonance_ws_connections Live WebSocket connections.\n\
         # TYPE resonance_ws_connections gauge\n\
         resonance_ws_connections {ws}\n",
        created   = m.pulses_created.load(Ordering::Relaxed),
        evaporated= m.pulses_evaporated.load(Ordering::Relaxed),
        echoes    = m.echoes.load(Ordering::Relaxed),
        saves     = m.saves.load(Ordering::Relaxed),
        comments  = m.comments.load(Ordering::Relaxed),
        auctions  = m.auctions_run.load(Ordering::Relaxed),
        juries    = m.juries_summoned.load(Ordering::Relaxed),
        ws        = m.ws_connections.load(Ordering::Relaxed),
    );

    (
        StatusCode::OK,
        [("Content-Type", "text/plain; version=0.0.4")],
        body,
    )
}
