// ============================================================================
// resonance-backend/src/state.rs
// Shared application state accessible from every Axum handler.
// ============================================================================

use redis::aio::ConnectionManager;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub redis: ConnectionManager,
    pub rtb: Arc<crate::handlers::rtb::RtbEngine>,
    pub moderator: Arc<crate::handlers::moderation::Moderator>,
}

impl AppState {
    pub fn new(db: PgPool, redis: ConnectionManager) -> Self {
        let platform_share_bps = std::env::var("RTB_PLATFORM_SHARE_BPS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3000);
        let rtb = Arc::new(crate::handlers::rtb::RtbEngine::new(platform_share_bps));
        let moderator = Arc::new(crate::handlers::moderation::Moderator::new());
        Self { db, redis, rtb, moderator }
    }
}
