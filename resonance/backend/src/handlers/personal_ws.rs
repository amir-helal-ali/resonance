// ============================================================================
// resonance-backend/src/handlers/personal_ws.rs
// Personal WebSocket endpoint: GET /ws/personal?user_id=...
//
// Subscribes to the Redis channel `user:{user_id}` and forwards every
// published event to the browser. Used for:
//   - dm:new
//   - pulse:interaction
//   - presence:pulse
//   - resonance:threshold
//   - jury:concluded
//   - goal:lit
// ============================================================================

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use redis::AsyncCommands;
use serde::Deserialize;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct PersonalWsQuery {
    pub user_id: Uuid,
}

/// GET /ws/personal?user_id=...
///
/// NOTE: in production you'd verify the Ed25519 signature in the WS
/// upgrade handshake (via a `Sec-WebSocket-Protocol` subprotocol header
/// or a signed `?token=` query param). For brevity here we accept any
/// caller that knows the user_id; rate limiting protects against abuse.
pub async fn personal_ws(
    State(state): State<AppState>,
    Query(q): Query<PersonalWsQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| personal_ws_task(socket, state, q.user_id))
}

async fn personal_ws_task(socket: WebSocket, state: AppState, user_id: Uuid) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to this user's personal channel.
    let mut pubsub = state.redis.clone();
    let mut sub = pubsub.as_pubsub();
    let channel = format!("user:{}", user_id);
    if let Err(e) = sub.subscribe(&channel).await {
        tracing::error!(error = ?e, %channel, "personal ws subscribe failed");
        return;
    }

    // Heartbeat.
    let (hb_tx, mut hb_rx) = tokio::sync::mpsc::channel::<Message>(8);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        interval.tick().await;
        loop {
            interval.tick().await;
            if hb_tx.send(Message::Ping(vec![1, 2, 3])).await.is_err() {
                break;
            }
        }
    });

    tracing::info!(%user_id, "personal ws connected");

    loop {
        tokio::select! {
            msg = sub.on_message() => {
                let payload = match msg {
                    Ok(m) => m.get_payload::<String>().unwrap_or_default(),
                    Err(e) => {
                        tracing::warn!(error = ?e, "personal pubsub recv error");
                        break;
                    }
                };
                if sender.send(Message::Text(payload)).await.is_err() {
                    break;
                }
            }
            Some(msg) = hb_rx.recv() => {
                if sender.send(msg).await.is_err() {
                    break;
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }

    tracing::info!(%user_id, "personal ws disconnected");
}
