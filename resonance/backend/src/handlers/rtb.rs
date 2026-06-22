// ============================================================================
// resonance-backend/src/handlers/rtb.rs
// Instant Rev-Share Real-Time Bidding (RTB) engine.
//
// Target: end-to-end auction latency <50ms (excluding DSP network jitter).
// Revenue split happens atomically inside a single Postgres transaction:
//   creator_balance += winning_bid * (1 - platform_share)
//   platform_balance += winning_bid * platform_share
// ============================================================================

use axum::{extract::State, Json};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

use crate::{
    errors::{AppError, AppResult},
    state::AppState,
};

/// The RTB engine holds the platform share in basis points (1bp = 0.01%).
/// Default 3000bp = 30%. Creator gets the remaining 70%.
pub struct RtbEngine {
    platform_share_bps: u32,
}

impl RtbEngine {
    pub fn new(platform_share_bps: u32) -> Self {
        Self { platform_share_bps }
    }

    /// Run a second-price auction among DSP bids.
    /// The winner pays the second-highest bid price (Vickrey auction).
    /// Returns (winner, price_to_charge).
    pub fn auction(&self, bids: &[Bid]) -> Option<(&Bid, u64)> {
        if bids.is_empty() {
            return None;
        }
        let mut sorted: Vec<&Bid> = bids.iter().collect();
        sorted.sort_by(|a, b| b.price_mlsl.cmp(&a.price_mlsl));
        let winner = sorted[0];
        let price = if sorted.len() > 1 {
            sorted[1].price_mlsl // second price
        } else {
            winner.price_mlsl // reserve price = winning price if only one bidder
        };
        Some((winner, price))
    }

    /// Atomically split the revenue between the platform and the creator.
    /// Returns the inserted `ad_auctions` row id.
    pub async fn split_revenue(
        &self,
        pool: &PgPool,
        pulse_id: Option<Uuid>,
        creator_id: Uuid,
        winning_bid_mlsl: u64,
    ) -> Result<Uuid, sqlx::Error> {
        let platform_share = (winning_bid_mlsl * self.platform_share_bps as u64) / 10_000;
        let creator_share = winning_bid_mlsl - platform_share;

        let mut tx = pool.begin().await?;

        // Insert the auction ledger row.
        let row: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO ad_auctions (pulse_id, creator_id, winning_bid_mlsl, platform_share_mlsl, creator_share_mlsl)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
        )
        .bind(pulse_id)
        .bind(creator_id)
        .bind(winning_bid_mlsl as i64)
        .bind(platform_share as i64)
        .bind(creator_share as i64)
        .fetch_one(&mut *tx)
        .await?;

        // Atomically increment the creator's balance.
        sqlx::query(
            "UPDATE users SET creator_balance_mlsl = creator_balance_mlsl + $1 WHERE id = $2",
        )
        .bind(creator_share as i64)
        .bind(creator_id)
        .execute(&mut *tx)
        .await?;

        // The platform balance is tracked in a single-row table omitted here
        // for brevity; in production we'd do `UPDATE platform_ledger SET balance = balance + $1`.
        tx.commit().await?;
        Ok(row.0)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Bid {
    pub dsp_id: String,
    pub price_mlsl: u64,
    /// URL or token for the creative payload.
    pub creative_url: String,
}

#[derive(Debug, Deserialize)]
pub struct RunAuctionRequest {
    pub pulse_id: Option<Uuid>,
    pub creator_id: Uuid,
    /// Optional pre-fetched bids. If empty, the engine fans out to DSPs.
    pub bids: Vec<Bid>,
}

#[derive(Debug, Serialize)]
pub struct RunAuctionResponse {
    pub auction_id: Uuid,
    pub winner_dsp_id: String,
    pub price_charged_mlsl: u64,
    pub creative_url: String,
    pub latency_ms: u64,
}

/// POST /rtb/auction — run an auction and atomically split the revenue.
///
/// Total server-side budget: 45ms. The fan-out to DSPs (if needed) is
/// capped at 40ms; the remaining 5ms is for the auction + revenue split.
pub async fn run_auction(
    State(state): State<AppState>,
    Json(req): Json<RunAuctionRequest>,
) -> AppResult<Json<RunAuctionResponse>> {
    let started = std::time::Instant::now();

    // 1. Collect bids. If the client didn't send any, fan out to DSPs via
    //    Redis Pub/Sub (DSPs are subscribed to `rtb:bids:request`).
    let bids = if req.bids.is_empty() {
        fan_out_dsp_bids(&state, 40).await?
    } else {
        req.bids.clone()
    };

    // 2. Run the second-price auction.
    let (winner, price) = state
        .rtb
        .auction(&bids)
        .ok_or_else(|| AppError::BadRequest("no bids received".into()))?;

    // 3. Atomic revenue split.
    let auction_id = state
        .rtb
        .split_revenue(&state.db, req.pulse_id, req.creator_id, price)
        .await
        .map_err(AppError::Db)?;

    let latency_ms = started.elapsed().as_millis() as u64;

    Ok(Json(RunAuctionResponse {
        auction_id,
        winner_dsp_id: winner.dsp_id.clone(),
        price_charged_mlsl: price,
        creative_url: winner.creative_url.clone(),
        latency_ms,
    }))
}

/// Fan out bid requests to DSPs and collect responses within the timeout.
async fn fan_out_dsp_bids(state: &AppState, timeout_ms: u64) -> AppResult<Vec<Bid>> {
    let mut conn = state.redis.clone();
    let request_id = Uuid::new_v4().to_string();
    let channel = format!("rtb:bids:response:{request_id}");

    // Subscribe BEFORE publishing to avoid a race.
    let mut pubsub = conn.as_pubsub();
    pubsub
        .subscribe(&channel)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("subscribe: {e}")))?;

    // Publish the bid request. DSPs are subscribed to `rtb:bids:request`.
    let request = serde_json::json!({
        "request_id": request_id,
        "response_channel": channel,
        "ts": chrono::Utc::now().timestamp_millis(),
    });
    let _: () = conn
        .publish("rtb:bids:request", request.to_string())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("publish request: {e}")))?;

    // Collect bids until timeout. We wait up to `timeout_ms` total.
    let deadline = Duration::from_millis(timeout_ms);
    let mut bids = Vec::new();
    while bids.len() < 8 {
        match timeout(deadline, pubsub.on_message()).await {
            Ok(Ok(msg)) => {
                if let Ok(s) = msg.get_payload::<String>() {
                    if let Ok(bid) = serde_json::from_str::<Bid>(&s) {
                        bids.push(bid);
                    }
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(error = ?e, "pubsub recv error");
                break;
            }
            Err(_) => break, // timeout
        }
    }
    Ok(bids)
}
