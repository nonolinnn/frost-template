use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Database row types
// ---------------------------------------------------------------------------

/// Row from the `signing_nonces` table.
#[derive(Debug, Clone, FromRow)]
pub struct SigningNoncesRow {
    pub id: Uuid,
    pub signing_request_id: Uuid,
    pub nonces: serde_json::Value,
    pub created_at: OffsetDateTime,
}

// ---------------------------------------------------------------------------
// API request types
// ---------------------------------------------------------------------------

/// Request body for `POST /api/signing/round1`.
#[derive(Debug, Deserialize)]
pub struct SigningRound1Request {
    pub signing_request_id: Uuid,
    pub wallet_index: i32,
}

/// Request body for `POST /api/signing/round2`.
#[derive(Debug, Deserialize)]
pub struct SigningRound2Request {
    pub signing_request_id: Uuid,
    pub wallet_index: i32,
    pub message: String,
    pub commitments: std::collections::HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// API response types
// ---------------------------------------------------------------------------

/// Response for `POST /api/signing/round1`.
#[derive(Debug, Serialize)]
pub struct SigningRound1Response {
    pub node_id: String,
    pub signing_request_id: Uuid,
    pub commitments: serde_json::Value,
}

/// Response for `POST /api/signing/round2`.
#[derive(Debug, Serialize)]
pub struct SigningRound2Response {
    pub node_id: String,
    pub signing_request_id: Uuid,
    pub signature_share: serde_json::Value,
}
