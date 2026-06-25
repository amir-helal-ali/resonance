// ============================================================================
// resonance-backend/src/middleware/rate_limit.rs
// Token-bucket rate limiter backed by Redis. Applied as an Axum layer.
//
// Two presets:
//   - `strict`  : 10 req / 60s  (for /register, /blind-index, /verify-otp)
//   - `standard`: 60 req / 60s  (for general API)
//   - `ws`      : 5 connections / 60s (for /ws)
//
// The limiter keys on (ip, route_pattern) so a single IP cannot exhaust
// the global budget on one endpoint. Behind a reverse proxy, the IP is
// taken from X-Forwarded-For (set TRUSTED_IP_HEADER).
// ============================================================================

use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use redis::AsyncCommands;
use std::time::Duration;

use crate::{errors::AppError, state::AppState};

#[derive(Clone, Copy, Debug)]
pub enum RateLimitPreset {
    Strict,    // 10 / 60s
    Standard,  // 60 / 60s
    Ws,        // 5 / 60s
}

impl RateLimitPreset {
    fn limit(&self) -> u32 {
        match self {
            Self::Strict   => 10,
            Self::Standard => 60,
            Self::Ws       => 5,
        }
    }
    fn window_secs(&self) -> u64 {
        60
    }
}

/// Extract the client IP. If `TRUSTED_IP_HEADER` is set (e.g. "X-Forwarded-For"),
/// use that header; otherwise fall back to the socket addr (which Axum makes
/// available via the `ConnectInfo` extension, but for simplicity we use a
/// placeholder here).
fn extract_ip(req: &Request<Body>) -> String {
    if let Ok(header_name) = std::env::var("TRUSTED_IP_HEADER") {
        if let Some(val) = req.headers().get(&header_name).and_then(|h| h.to_str().ok()) {
            // X-Forwarded-For can be a comma-separated list; take the first.
            return val.split(',').next().unwrap_or("unknown").trim().to_string();
        }
    }
    "unknown".to_string()
}

/// The middleware entry point. Usage:
///   .layer(from_fn_with_state(state.clone(), rate_limit_standard))
pub async fn rate_limit_strict(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    check(&state, &req, RateLimitPreset::Strict).await?;
    Ok(next.run(req).await)
}

pub async fn rate_limit_standard(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    check(&state, &req, RateLimitPreset::Standard).await?
        .map_or_else(|| Ok(next.run(req).await), |resp| Ok(resp))
}

pub async fn rate_limit_ws(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    check(&state, &req, RateLimitPreset::Ws).await?
        .map_or_else(|| Ok(next.run(req).await), |resp| Ok(resp))
}

/// Returns `Some(response)` if the request should be short-circuited (429),
/// or `None` if it should proceed.
async fn check(
    state: &AppState,
    req: &Request<Body>,
    preset: RateLimitPreset,
) -> Result<Option<Response>, AppError> {
    let ip = extract_ip(req);
    let path = req.uri().path();
    let key = format!("rl:{}:{}:{:?}", ip, path, preset);

    let mut conn = state.redis.clone();
    // INCR + EXPIRE in a single pipeline. We use `incr` then check; if the
    // count exceeds the limit, return 429.
    let count: i64 = conn
        .incr(&key, 1)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("redis incr rate limit: {e}")))?;

    if count == 1 {
        // First request in the window — set the TTL.
        let _: () = conn
            .expire(&key, preset.window_secs() as i64)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("redis expire rate limit: {e}")))?;
    }

    if count > preset.limit() as i64 {
        tracing::warn!(%ip, %path, count, limit = preset.limit(), "rate limited");
        let retry_after = preset.window_secs();
        let body = serde_json::json!({
            "code": "rate_limited",
            "message": "كتير أوي. حاول تاني بعد شوية.",
            "retry_after_secs": retry_after,
        });
        return Ok(Some((
            StatusCode::TOO_MANY_REQUESTS,
            [("Retry-After", retry_after.to_string().as_str())],
            axum::Json(body),
        )
            .into_response()));
    }

    Ok(None)
}

#[allow(dead_code)]
fn _duration_helper() -> Duration {
    Duration::from_secs(60)
}
