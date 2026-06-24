// ============================================================================
// resonance-backend/src/handlers/moderation.rs
// Hybrid moderation: AI scoring via `candle` + Thermodynamic Cooling.
//
// Two paths:
//   1. SERVER-SIDE SCORING (production): the moderation worker decrypts the
//      pulse via a per-user KEK held in a TEE, then runs the BERT-tiny
//      toxicity classifier via `candle-transformers`. This file shows the
//      full integration scaffold with `candle_core`, `candle_nn`, and
//      `candle_transformers::models::bert` loaded lazily behind a `Mutex`.
//
//   2. HEURISTIC FALLBACK (dev): when no model file is present at
//      `/app/models/toxicity`, we fall back to a keyword heuristic so the
//      pipeline can be exercised end-to-end without downloading weights.
// ============================================================================

use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::db::queries;

/// The moderator holds the loaded model behind a `Mutex` so we load it
/// exactly once. In production you'd use a dedicated inference thread pool
/// (rayon) and a `OnceCell<Hub>` for the `candle` device.
pub struct Moderator {
    inner: Mutex<Inner>,
}

struct Inner {
    /// The loaded model. `None` when the weights file is absent (dev mode).
    /// When `Some`, this is `Box<dyn ToxicityModel + Send + Sync>`.
    model: Option<Box<dyn ToxicityModel + Send + Sync>>,
    /// Whether we've already attempted to load the model (so we don't retry
    /// every call).
    load_attempted: bool,
}

/// Trait abstracting the toxicity model. Two impls:
///   - `CandleToxicityModel` (production)
///   - `HeuristicToxicityModel` (dev fallback)
pub trait ToxicityModel: Send + Sync {
    fn score(&self, text: &str) -> f32;
}

impl Moderator {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                model: None,
                load_attempted: false,
            }),
        }
    }

    /// Score a piece of text for toxicity. Returns a probability in [0,1].
    pub async fn score_toxicity(&self, text: &str) -> f32 {
        let mut guard = self.inner.lock().await;

        // Lazy-load the model on first use.
        if !guard.load_attempted {
            guard.load_attempted = true;
            match load_model().await {
                Ok(m) => {
                    tracing::info!("toxicity model loaded");
                    guard.model = Some(Box::new(m));
                }
                Err(e) => {
                    tracing::warn!(error = ?e, "model load failed; using heuristic");
                    guard.model = Some(Box::new(HeuristicToxicityModel));
                }
            }
        }

        guard
            .model
            .as_ref()
            .map(|m| m.score(text))
            .unwrap_or_else(|| HeuristicToxicityModel.score(text))
    }

    /// Evaluate a pulse's toxicity, store the verdict, and apply cooling
    /// if needed. Called from a tokio::spawn in the pulse handler.
    pub async fn evaluate_and_store(
        &self,
        pool: &PgPool,
        pulse_id: Uuid,
    ) -> Result<(), crate::errors::AppError> {
        // 1. Fetch the pulse ciphertext. In production this would be
        //    decrypted via a TEE worker first; for the skeleton we apply
        //    the model to the base64 representation (obviously wrong, but
        //    exercises the pipeline).
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

        // 4. If cooling was applied, the cron will summon a jury when
        //    toxicity ≥ 0.9 (see cron job #6).
        if moderation.verdict == "cooling" {
            tracing::info!(%pulse_id, until = ?moderation.cooling_until, "pulse cooling applied");
        }

        Ok(())
    }
}

/// Load the toxicity model. Returns the `candle`-backed impl if the weights
/// file exists; otherwise returns an error so the caller falls back to the
/// heuristic.
async fn load_model() -> Result<CandleToxicityModel, anyhow::Error> {
    let model_path = std::env::var("TOXICITY_MODEL_PATH")
        .unwrap_or_else(|_| "/app/models/toxicity/model.safetensors".into());
    let tokenizer_path = std::env::var("TOXICITY_TOKENIZER_PATH")
        .unwrap_or_else(|_| "/app/models/toxicity/tokenizer.json".into());

    if !std::path::Path::new(&model_path).exists() {
        anyhow::bail!("model file not found at {}", model_path);
    }

    // We construct the model on a background thread because `candle`'s
    // `Device::Cpu` construction is sync. In production you'd use
    // `Device::cuda_if_available(0)` and a dedicated inference thread pool.
    let model = tokio::task::spawn_blocking(move || -> Result<CandleToxicityModel, anyhow::Error> {
        use candle_core::{Device, Tensor};
        use candle_transformers::models::bert::{BertModel, Config};
        use std::sync::Arc;

        // 1. Load the tokenizer.
        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("tokenizer load: {e}"))?;

        // 2. Load the model weights.
        let vb = candle_nn::VarBuilder::from_mmaped_safetensors(
            &[model_path.clone()],
            candle_core::DType::F32,
            &Device::Cpu,
        )?;
        let config = Config::default();
        let model = BertModel::load(vb, &config)?;
        let model = Arc::new(model);

        // Sanity: a dummy forward pass to ensure the weights are valid.
        let dummy = Tensor::zeros((1, 8), candle_core::DType::U32, &Device::Cpu)?;
        let _ = model.forward(&dummy, &dummy)?;

        Ok(CandleToxicityModel {
            model,
            tokenizer,
            _phantom: (),
        })
    })
    .await??;

    Ok(model)
}

/// Production toxicity model backed by `candle` + BERT-tiny.
pub struct CandleToxicityModel {
    model: std::sync::Arc<candle_transformers::models::bert::BertModel>,
    tokenizer: tokenizers::Tokenizer,
    _phantom: (),
}

impl ToxicityModel for CandleToxicityModel {
    fn score(&self, text: &str) -> f32 {
        // Tokenize → forward pass → sigmoid on the [CLS] logit.
        use candle_core::Tensor;
        let enc = match self.tokenizer.encode(text, true) {
            Ok(e) => e,
            Err(_) => return 0.0,
        };
        let ids = enc.get_ids();
        let attn = enc.get_attention_mask();
        let ids_t = match Tensor::from_slice(ids.as_u32_buffer(), (1, ids.len()), &candle_core::Device::Cpu) {
            Ok(t) => t,
            Err(_) => return 0.0,
        };
        let attn_t = match Tensor::from_slice(
            attn.as_u32_buffer(),
            (1, attn.len()),
            &candle_core::Device::Cpu,
        ) {
            Ok(t) => t,
            Err(_) => return 0.0,
        };

        let logits = match self.model.forward(&ids_t, &attn_t) {
            Ok(l) => l,
            Err(_) => return 0.0,
        };

        // Take the [CLS] token's hidden state (index 0).
        let cls = match logits.get(0).and_then(|t| t.get(0)) {
            Ok(c) => c,
            Err(_) => return 0.0,
        };

        // Apply a linear probe (in production you'd load a separate
        // classification head). For the skeleton: sigmoid(mean(cls)).
        let mean = match cls.mean(0) {
            Ok(m) => m,
            Err(_) => return 0.0,
        };
        let scalar = match mean.to_vec1::<f32>() {
            Ok(v) => v.first().copied().unwrap_or(0.0),
            Err(_) => return 0.0,
        };
        // Sigmoid.
        1.0 / (1.0 + (-scalar).exp())
    }
}

/// Dev fallback: keyword-based heuristic. Used when no model weights are present.
pub struct HeuristicToxicityModel;
impl ToxicityModel for HeuristicToxicityModel {
    fn score(&self, text: &str) -> f32 {
        heuristic_toxicity(text)
    }
}

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
