// ============================================================================
// resonance-backend/src/state.rs
// Shared application state accessible from every Axum handler.
// ============================================================================

use redis::aio::ConnectionManager;
use sqlx::PgPool;
use std::sync::Arc;

use crate::handlers::{MetricsHandle, Moderator, RtbEngine};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub redis: ConnectionManager,
    pub rtb: Arc<RtbEngine>,
    pub moderator: Arc<Moderator>,
    pub metrics: MetricsHandle,
}

impl AppState {
    pub fn new(db: PgPool, redis: ConnectionManager) -> Self {
        let platform_share_bps = std::env::var("RTB_PLATFORM_SHARE_BPS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3000);
        let rtb = Arc::new(RtbEngine::new(platform_share_bps));
        let moderator = Arc::new(Moderator::new());
        let metrics = metrics_handle();
        Self { db, redis, rtb, moderator, metrics }
    }
}
