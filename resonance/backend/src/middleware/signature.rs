// ============================================================================
// resonance-backend/src/middleware/signature.rs
// Axum middleware: verify an Ed25519 signature on every mutating request.
//
// Header contract (sent by the SvelteKit frontend):
//   X-Resonance-Key   : base64(ed25519 public key, 32 bytes)
//   X-Resonance-Ts    : unix milliseconds (string)
//   X-Resonance-Sig   : base64(ed25519 signature, 64 bytes)
//
// The canonical signing string is identical to what the frontend signs:
//   method || "\n" || path || "\n" || timestamp || "\n" || sha256(body)
//
// Replay protection: timestamp must be within ±60s of server time.
// ============================================================================

use axum::{
    body::Body,
    extract::{Request, State},
    http::{self, StatusCode},
    middleware::Next,
    response::Response,
};
use base64::Engine;
use chrono::Utc;
use tracing::warn;

use crate::{
    crypto::verify_ed25519_signature,
    errors::AppError,
    state::AppState,
};

/// Maximum allowed clock skew between client and server, in milliseconds.
const MAX_SKEW_MS: i64 = 60_000;

/// The middleware entry point. Usage:
///   use axum::middleware::from_fn_with_state;
///   Router::new()
///     .route("/pulses", post(create_pulse))
///     .layer(from_fn_with_state(state.clone(), signature_middleware))
pub async fn signature_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    // 1. Read headers. Missing header => 401.
    let key_b64 = req
        .headers()
        .get("X-Resonance-Key")
        .and_then(|h| h.to_str().ok())
        .ok_or(AppError::InvalidSignature)?;
    let ts_str = req
        .headers()
        .get("X-Resonance-Ts")
        .and_then(|h| h.to_str().ok())
        .ok_or(AppError::InvalidSignature)?;
    let sig_b64 = req
        .headers()
        .get("X-Resonance-Sig")
        .and_then(|h| h.to_str().ok())
        .ok_or(AppError::InvalidSignature)?;

    // 2. Decode base64. Use the URL-safe alphabet to match the browser side.
    let pubkey_bytes = base64::engine::general_purpose::STANDARD
        .decode(key_b64.as_bytes())
        .map_err(|_| AppError::InvalidSignature)?;
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(sig_b64.as_bytes())
        .map_err(|_| AppError::InvalidSignature)?;

    if pubkey_bytes.len() != 32 || sig_bytes.len() != 64 {
        return Err(AppError::InvalidSignature);
    }
    let mut pubkey = [0u8; 32];
    pubkey.copy_from_slice(&pubkey_bytes);
    let mut sig = [0u8; 64];
    sig.copy_from_slice(&sig_bytes);

    // 3. Parse timestamp and check skew.
    let ts_ms: i64 = ts_str
        .parse()
        .map_err(|_| AppError::InvalidSignature)?;
    let now_ms = Utc::now().timestamp_millis();
    if (now_ms - ts_ms).abs() > MAX_SKEW_MS {
        warn!(skew_ms = %((now_ms - ts_ms).abs()), "signature timestamp skew");
        return Err(AppError::InvalidSignature);
    }

    // 4. Buffer the body. We need its bytes to compute the canonical hash.
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let bytes = match axum::body::to_bytes(req.into_body(), 8 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return Err(AppError::BadRequest("body too large".into())),
    };

    // 5. Verify the Ed25519 signature.
    verify_ed25519_signature(&pubkey, method.as_str(), &path, ts_ms, &bytes, &sig)?;

    // 6. Re-attach the body and continue.
    req = Request::builder()
        .method(method)
        .uri(path)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(bytes))
        .map_err(|e| AppError::Internal(anyhow::anyhow!("rebuild request: {e}")))?;

    // Stash the verified pubkey in extensions so handlers can use it.
    req.extensions_mut().insert(VerifiedPubKey(pubkey));

    Ok(next.run(req).await)
}

/// Extension type carrying the verified Ed25519 public key.
/// Handlers extract this to identify the acting user.
#[derive(Debug, Clone, Copy)]
pub struct VerifiedPubKey(pub [u8; 32]);

/// Convenience extractor: look up the `users` row matching the verified pubkey.
pub async fn user_from_pubkey(
    pool: &sqlx::PgPool,
    pubkey: &[u8; 32],
) -> Result<Option<crate::db::models::UserRow>, sqlx::Error> {
    sqlx::query_as::<_, crate::db::models::UserRow>(
        "SELECT * FROM users WHERE public_key = $1",
    )
    .bind(pubkey)
    .fetch_optional(pool)
    .await
}

/// Convenience status-code mapping for the middleware (Axum's `IntoResponse`
/// already does this via `AppError`, but we want to short-circuit on the
/// `Body`-limit error specifically).
#[allow(dead_code)]
pub fn body_too_large() -> (StatusCode, &'static str) {
    (StatusCode::PAYLOAD_TOO_LARGE, "body too large")
}
