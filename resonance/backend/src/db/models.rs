// ============================================================================
// resonance-backend/src/db/models.rs
// Strongly-typed row models mirroring the DB schema.
// ============================================================================

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct UserRow {
    pub id: Uuid,
    pub username: String,
    pub email_ciphertext: Vec<u8>,
    pub email_blind_index: Vec<u8>,
    pub public_key: Vec<u8>,
    pub imprint: String,
    pub horizon: String,
    pub email_verified: bool,
    pub is_quarantined: bool,
    pub creator_balance_mlsl: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PulseRow {
    pub id: Uuid,
    pub author_id: Uuid,
    pub ciphertext: Vec<u8>,
    pub encryption_key_id: Uuid,
    pub lifecycle: String, // glow | linger | evaporated
    pub is_preserved: bool,
    pub last_interaction_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub evaporated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ConnectionRow {
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub resonance_score: i16,
    pub last_interaction_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TraceRow {
    pub id: Uuid,
    pub visited_id: Uuid,
    pub visitor_id: Option<Uuid>,
    pub kind: String, // anonymous | named
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ModerationRow {
    pub id: Uuid,
    pub pulse_id: Uuid,
    pub toxicity_score: f32,
    pub verdict: String,
    pub cooling_until: Option<DateTime<Utc>>,
    pub jury_summoned_at: Option<DateTime<Utc>>,
    pub jury_verdict: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
