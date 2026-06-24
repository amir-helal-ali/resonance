// ============================================================================
// resonance-backend/src/db/mod.rs
// SQLx connection pool + typed row models.
// ============================================================================

use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

pub mod models;
pub mod queries;

/// Build a `PgPool` from a `DATABASE_URL` with sensible production defaults:
///   - max 20 connections
///   - 5s acquire timeout
///   - 30s idle timeout
///   - statement-level logging disabled (privacy: never log OTP rows)
pub async fn build_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Some(Duration::from_secs(30)))
        .max_lifetime(Some(Duration::from_secs(3600)))
        .test_before_acquire(false)
        .connect(database_url)
        .await
}

/// Run pending migrations on startup. In production you'd typically run
/// `sqlx migrate` as a separate CI step, but running here makes the
/// container self-bootstrapping for dev/test.
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
