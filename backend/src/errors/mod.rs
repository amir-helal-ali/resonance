// ============================================================================
// resonance-backend/src/errors/mod.rs
// Centralized error handling. All fallible operations return `AppError`,
// which implements `IntoResponse` so Axum converts them to HTTP responses
// with the right status code and a privacy-safe JSON body.
// ============================================================================

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// The single error type for the entire backend.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid proof-of-work: difficulty not met")]
    InvalidPow,

    #[error("invalid or missing signature")]
    InvalidSignature,

    #[error("blind index already registered")]
    BlindIndexCollision,

    #[error("username already taken")]
    UsernameTaken,

    #[error("resource not found")]
    NotFound,

    #[error("unauthorized: resonance threshold not met")]
    Unauthorized,

    #[error("rate limited; try again later")]
    RateLimited,

    #[error("invalid request payload: {0}")]
    BadRequest(String),

    #[error("otp verification failed")]
    OtpFailed,

    #[error("moderation verdict: quarantined")]
    Quarantined,

    #[error("internal error")]
    Internal(#[from] anyhow::Error),

    #[error("database error")]
    Db(#[from] sqlx::Error),

    #[error("redis error")]
    Redis(#[from] redis::RedisError),

    #[error("cryptographic error")]
    Crypto(String),
}

// Convenience alias for handlers.
pub type AppResult<T> = Result<T, AppError>;

/// Convert `AppError` into an HTTP response. We deliberately DO NOT leak
/// internal error strings to clients — only a stable code + message.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, msg) = match &self {
            AppError::InvalidPow            => (StatusCode::BAD_REQUEST,             "invalid_pow",       self.to_string()),
            AppError::InvalidSignature      => (StatusCode::UNAUTHORIZED,            "invalid_signature", self.to_string()),
            AppError::BlindIndexCollision   => (StatusCode::CONFLICT,                "blind_collision",   self.to_string()),
            AppError::UsernameTaken         => (StatusCode::CONFLICT,                "username_taken",    self.to_string()),
            AppError::NotFound              => (StatusCode::NOT_FOUND,               "not_found",         self.to_string()),
            AppError::Unauthorized          => (StatusCode::FORBIDDEN,               "forbidden",         self.to_string()),
            AppError::RateLimited           => (StatusCode::TOO_MANY_REQUESTS,       "rate_limited",      self.to_string()),
            AppError::BadRequest(m)         => (StatusCode::BAD_REQUEST,             "bad_request",       m.clone()),
            AppError::OtpFailed             => (StatusCode::UNAUTHORIZED,            "otp_failed",        self.to_string()),
            AppError::Quarantined           => (StatusCode::FORBIDDEN,               "quarantined",       self.to_string()),
            AppError::Db(e)                 => {
                tracing::error!(error = ?e, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal", "internal error".into())
            }
            AppError::Redis(e)              => {
                tracing::error!(error = ?e, "redis error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal", "internal error".into())
            }
            AppError::Crypto(m)             => {
                tracing::error!(msg = %m, "crypto error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal", "internal error".into())
            }
            AppError::Internal(e)           => {
                tracing::error!(error = ?e, "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal", "internal error".into())
            }
        };

        let body = Json(json!({ "code": code, "message": msg }));
        (status, body).into_response()
    }
}

// `anyhow::Error` shim so we can use `?` on third-party fallible APIs.
pub mod anyhow {
    pub use ::anyhow::Error as AnyError;
}

// Re-export anyhow into the crate so handlers can write `AppError::Internal(anyhow::anyhow!(...).into())`.
// We add the dependency here for the shim above.
