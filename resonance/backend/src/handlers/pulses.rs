// ============================================================================
// resonance-backend/src/handlers/pulses.rs
// "نبضة" (pulse) creation + Live Gravity Feed over WebSockets.
// ============================================================================

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    Json,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::queries,
    errors::{AppError, AppResult},
    middleware::signature::VerifiedPubKey,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct CreatePulseRequest {
    /// AES-GCM ciphertext of the pulse body, base64-encoded.
    pub ciphertext: String,
    /// The wrapped symmetric key (encrypted under the user's HKDF KEK),
    /// base64-encoded. Server stores this opaquely.
    pub wrapped_key: String,
    /// Whether the user clicked "تخليد" (preserve).
    pub is_preserved: bool,
}

#[derive(Debug, Serialize)]
pub struct CreatePulseResponse {
    pub pulse_id: Uuid,
    pub lifecycle: &'static str,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// POST /pulses — create a new "نبضة".
///
/// The body is opaque ciphertext; the server is structurally blind to content.
/// After persistence we:
///   1. Insert an `encryption_keys` row for the wrapped key.
///   2. Insert the `pulses` row.
///   3. ZADD the pulse into the `feed:glow` ZSET (score = now_ms).
///   4. PUBLISH `feed:new` with the pulse id, which all WS clients receive.
///   5. Trigger async AI moderation.
pub async fn create_pulse(
    State(state): State<AppState>,
    VerifiedPubKey(pubkey): VerifiedPubKey,
    Json(req): Json<CreatePulseRequest>,
) -> AppResult<Json<CreatePulseResponse>> {
    // 1. Resolve the user from the verified pubkey.
    let user = crate::middleware::signature::user_from_pubkey(&state.db, &pubkey)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::InvalidSignature)?;
    if user.is_quarantined {
        return Err(AppError::Quarantined);
    }

    // 2. Decode ciphertext + wrapped key.
    let ciphertext = base64::decode(req.ciphertext.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("ciphertext not base64: {e}")))?;
    let wrapped_key = base64::decode(req.wrapped_key.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("wrapped_key not base64: {e}")))?;

    // 3. Insert the encryption key (the wrapped form is opaque to us).
    let key_row: (Uuid,) = sqlx::query_as(
        "INSERT INTO encryption_keys (wrapped_key) VALUES ($1) RETURNING id",
    )
    .bind(&wrapped_key)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Db)?;
    let encryption_key_id = key_row.0;

    // 4. Insert the pulse.
    let pulse = queries::insert_pulse(&state.db, user.id, &ciphertext, encryption_key_id)
        .await
        .map_err(AppError::Db)?;

    // 5. Set is_preserved if requested.
    if req.is_preserved {
        sqlx::query("UPDATE pulses SET is_preserved = true WHERE id = $1")
            .bind(pulse.id)
            .execute(&state.db)
            .await
            .map_err(AppError::Db)?;
    }

    // 6. Add to the live "glow" feed (ZSET in Redis).
    let now_ms = chrono::Utc::now().timestamp_millis() as f64;
    let mut conn = state.redis.clone();
    let _: () = conn
        .zadd("feed:glow", pulse.id.to_string(), now_ms)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("zadd feed:glow: {e}")))?;

    // 7. Notify all WS subscribers. The payload includes the author username
    //    so clients can render immediately without an extra round-trip.
    let event = serde_json::json!({
        "type": "pulse:new",
        "pulse_id": pulse.id,
        "author": user.username,
        "created_at": pulse.created_at,
    });
    let _: () = conn
        .publish("feed:new", event.to_string())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("publish feed:new: {e}")))?;

    // 8. Spawn async moderation. We do NOT block the request on this; the
    //    pulse is already live. If moderation later quarantines it, the
    //    cooling event is broadcast via the same Redis Pub/Sub channel.
    let moderator = state.moderator.clone();
    let pulse_id = pulse.id;
    let db = state.db.clone();
    tokio::spawn(async move {
        if let Err(e) = moderator.evaluate_and_store(&db, pulse_id).await {
            tracing::error!(error = ?e, %pulse_id, "moderation failed");
        }
    });

    Ok(Json(CreatePulseResponse {
        pulse_id: pulse.id,
        lifecycle: "glow",
        created_at: pulse.created_at,
    }))
}

/// GET /feed/glow — fetch the current glow feed (top 50).
pub async fn get_glow_feed(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<GlowPulse>>> {
    let mut conn = state.redis.clone();
    // ZREVRANGE returns highest scores first = most recent.
    let ids: Vec<String> = conn
        .zrevrange("feed:glow", 0, 49)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("zrevrange: {e}")))?;

    if ids.is_empty() {
        return Ok(Json(vec![]));
    }

    // Fetch the pulse rows. We use a manual IN clause via unnest.
    let uuids: Vec<Uuid> = ids
        .iter()
        .filter_map(|s| Uuid::parse_str(s).ok())
        .collect();
    let rows: Vec<(Uuid, Uuid, Vec<u8>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT id, author_id, ciphertext, created_at
        FROM pulses
        WHERE id = ANY($1) AND lifecycle = 'glow'
        "#,
    )
    .bind(&uuids)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    let out: Vec<GlowPulse> = rows
        .into_iter()
        .map(|(id, author_id, ciphertext, created_at)| GlowPulse {
            pulse_id: id,
            author_id,
            ciphertext_b64: base64::encode(&ciphertext),
            created_at,
        })
        .collect();
    Ok(Json(out))
}

#[derive(Debug, Serialize)]
pub struct GlowPulse {
    pub pulse_id: Uuid,
    pub author_id: Uuid,
    pub ciphertext_b64: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// GET /ws — Live Gravity Feed WebSocket.
///
/// On connect the client joins the `feed:new` Pub/Sub channel. Every new
/// pulse is pushed immediately. We also send a heartbeat every 30s so
/// proxies don't kill idle connections.
pub async fn feed_ws(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| feed_ws_task(socket, state))
}

async fn feed_ws_task(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to the feed:new channel.
    let mut pubsub = state.redis.clone();
    let mut sub = pubsub.as_pubsub();
    if let Err(e) = sub.subscribe("feed:new").await {
        tracing::error!(error = ?e, "ws subscribe failed");
        return;
    }

    // Spawn a heartbeat task.
    let (hb_tx, mut hb_rx) = tokio::sync::mpsc::channel::<Message>(8);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        interval.tick().await; // immediate first tick skipped
        loop {
            interval.tick().await;
            if hb_tx.send(Message::Ping(vec![1, 2, 3])).await.is_err() {
                break;
            }
        }
    });

    // Main loop: forward Pub/Sub messages + heartbeats to the client.
    loop {
        tokio::select! {
            // Forward Pub/Sub messages.
            msg = sub.on_message() => {
                let payload = match msg {
                    Ok(m) => m.get_payload::<String>().unwrap_or_default(),
                    Err(e) => {
                        tracing::warn!(error = ?e, "pubsub recv error");
                        break;
                    }
                };
                if sender.send(Message::Text(payload)).await.is_err() {
                    break;
                }
            }
            // Forward heartbeats.
            Some(msg) = hb_rx.recv() => {
                if sender.send(msg).await.is_err() {
                    break;
                }
            }
            // Handle client-side messages (mostly close).
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}
