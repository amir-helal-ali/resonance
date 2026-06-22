// ============================================================================
// resonance-backend/src/main.rs
// صدى — Resonance backend entry point.
// ============================================================================

mod cron;
mod crypto;
mod db;
mod errors;
mod handlers;
mod middleware;
mod presence;
mod state;

use axum::{
    middleware::from_fn_with_state,
    routing::{get, post},
    Router,
};
use redis::aio::ConnectionManager;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ----- Logging -----
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("resonance=info,axum=info")),
        )
        .with_target(true)
        .json()
        .init();

    info!("صدى (Resonance) backend starting up");

    // ----- Configuration -----
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());

    // ----- DB pool -----
    let pool: PgPool = db::build_pool(&database_url).await?;
    db::run_migrations(&pool).await?;
    info!("database connected + migrations applied");

    // ----- Redis connection manager -----
    let redis_client = redis::Client::open(redis_url.as_str())?;
    let redis_conn = ConnectionManager::new(redis_client).await?;
    info!("redis connected");

    // ----- AppState -----
    let state = AppState::new(pool.clone(), redis_conn);

    // ----- Cron scheduler -----
    let _scheduler = Arc::new(cron::start(pool.clone()).await?);

    // ----- Router -----
    let app = build_router(state.clone());

    // ----- Server -----
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!(addr = %bind_addr, "server listening");

    let shutdown = async move {
        let _ = tokio::signal::ctrl_c().await;
        info!("ctrl-c received, shutting down");
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;

    info!("صدى backend shut down cleanly");
    Ok(())
}

/// Build the full Axum router. Routes that require a signature are wrapped
/// in the signature middleware; public routes (health, register, pow) are not.
fn build_router(state: AppState) -> Router {
    let public_routes = Router::new()
        .route("/health", get(health))
        .route("/pow/challenge", get(handlers::vault::issue_pow_challenge))
        .route("/register", post(handlers::vault::register))
        .route("/verify-otp", post(handlers::vault::verify_otp))
        .route("/blind-index", post(handlers::blind_index::compute_blind_index_handler))
        .route("/feed/glow", get(handlers::pulses::get_glow_feed))
        .route("/ws", get(handlers::pulses::feed_ws));

    let protected_routes = Router::new()
        // Pulses + interactions
        .route("/pulses", post(handlers::pulses::create_pulse))
        .route("/pulses/:id/echo",   post(handlers::interactions::echo))
        .route("/pulses/:id/save",   post(handlers::interactions::save))
        .route("/pulses/:id/comment",post(handlers::interactions::comment))
        .route("/pulses/:id/report", post(handlers::interactions::report))
        // Connections (resonance sync)
        .route("/connections/sync",        post(handlers::connections::sync))
        .route("/connections",             get(handlers::connections::list_my_connections))
        .route("/connections/suggest",     get(handlers::connections::suggest))
        .route("/connections/:target",     axum::routing::delete(handlers::connections::unsync))
        // Presence & traces
        .route("/presence/pulse",    post(presence::pulse_presence))
        .route("/presence/:user_id", get(presence::list_presence))
        .route("/traces",            get(presence::list_my_traces))
        // Goals (شموع الدعم)
        .route("/goals",             post(handlers::goals::create_goal))
        .route("/goals/:user_id",    get(handlers::goals::list_goals))
        .route("/goals/:id/light",   post(handlers::goals::light_candle))
        // Transient Jury
        .route("/jury/summoned",     get(handlers::jury::list_summoned))
        .route("/jury/:panel_id/vote", post(handlers::jury::cast_vote))
        // RTB
        .route("/rtb/auction",       post(handlers::rtb::run_auction))
        .layer(from_fn_with_state(
            state.clone(),
            middleware::signature::signature_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(CorsLayer::very_permissive()) // tighten in production
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// GET /health — liveness probe.
async fn health() -> &'static str {
    "صدى alive"
}
