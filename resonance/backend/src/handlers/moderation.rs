// ============================================================================
// resonance-backend/src/handlers/moderation.rs
// Hybrid moderation: AI scoring via `candle` + Thermodynamic Cooling.
//
// The AI model is loaded lazily on first use. We use a small BERT-tiny
// toxicity classifier; in production you'd swap for a fine-tuned model.
// Cooling duration scales with toxicity: hotter = longer cool.
// ============================================================================

use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::db::queries;

/// The moderator holds the loaded model behind a `Mutex` so we load it
/// exactly once. In production you'd use `OnceCell` and a dedicated
/// inference thread pool.
pub struct Moderator {
    inner: Mutex<Inner>,
}

struct Inner {
    /// Placeholder for the loaded candle model. We keep this as `Option<()>`
    /// to avoid pulling in heavyweight model-loading code in this skeleton;
    /// the actual `candle` integration is sketched in `score_toxicity`.
    model: Option<()>,
}

impl Moderator {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner { model: None }),
        }
    }

    /// Score a piece of text for toxicity. Returns a probability in [0,1].
    ///
    /// In production this would:
    ///   1. Tokenize the text with `tokenizers`.
    ///   2. Run forward pass through a BERT-tiny model via `candle-transformers`.
    ///   3. Apply sigmoid to the final logits.
    ///
    /// For this skeleton we use a heuristic that flags obvious slurs.
    /// The contract (input: ciphertext-decrypted text; output: f32) is
    /// the same — the caller doesn't care how the score is computed.
    pub async fn score_toxicity(&self, text: &str) -> f32 {
        let _guard = self.inner.lock().await;
        // Lazy-load the model on first use.
        if self.inner.lock().await.model.is_none() {
            // In production: load model weights from /app/models/toxicity.pt
            // self.inner.lock().await.model = Some(load_model().await?);
        }
        heuristic_toxicity(text)
    }

    /// Evaluate a pulse's toxicity, store the verdict, and apply cooling
    /// if needed. Called from a tokio::spawn in the pulse handler.
    pub async fn evaluate_and_store(
        &self,
        pool: &PgPool,
        pulse_id: Uuid,
    ) -> Result<(), crate::errors::AppError> {
        // 1. Fetch the pulse ciphertext. In a real deployment we'd need to
        //    decrypt it first (via a TEE or by delegating to a moderation
        //    worker that holds a per-user KEK). For this skeleton we just
        //    read the ciphertext bytes and apply the heuristic on the base64
        //    representation — obviously wrong, but it exercises the pipeline.
        let row: Option<(Vec<u8>,)> = sqlx::query_as("SELECT ciphertext FROM pulses WHERE id = $1")
            .bind(pulse_id)
            .fetch_optional(pool)
            .await
            .map_err(crate::errors::AppError::Db)?;
        let ciphertext = match row {
            Some((c,)) => c,
            None => return Ok(()),
        };
        let text = base64::encode(&ciphertext);

        // 2. Score.
        let score = self.score_toxicity(&text).await;
        tracing::info!(%pulse_id, score, "moderation score");

        // 3. Persist verdict + apply cooling window.
        let moderation = queries::insert_moderation_row(pool, pulse_id, score)
            .await
            .map_err(crate::errors::AppError::Db)?;

        // 4. If cooling was applied, broadcast to the live feed so clients
        //    can render the cooling state immediately.
        if moderation.verdict == "cooling" {
            // The broadcast itself would go through Redis Pub/Sub. Skipped here.
            tracing::info!(%pulse_id, until = ?moderation.cooling_until, "pulse cooling applied");
        }

        Ok(())
    }
}

/// A crude placeholder toxicity heuristic. Real implementation uses `candle`.
fn heuristic_toxicity(text: &str) -> f32 {
    let lower = text.to_lowercase();
    let bad_words = ["hate", "kill", "stupid", "idiot", "slur"];
    let mut hits = 0;
    for w in &bad_words {
        if lower.contains(w) {
            hits += 1;
        }
    }
    match hits {
        0 => 0.05,
        1 => 0.55,
        2 => 0.80,
        _ => 0.95,
    }
}
