// ============================================================================
// resonance-backend/src/lib.rs
// Library facade — exposes internal modules for integration tests.
// The main binary lives in `src/main.rs`.
// ============================================================================

pub mod cron;
pub mod crypto;
pub mod db;
pub mod errors;
pub mod handlers;
pub mod middleware;
pub mod presence;
pub mod state;
