use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Database row types
// ---------------------------------------------------------------------------

/// Row from the `signing_requests` table.
#[derive(Debug, Clone, FromRow)]
pub struct SigningRequestRow {
    pub id: Uuid,
    pub wallet_index: i32,
    pub recipient: String,
    pub amount_lamports: i64,
    pub status: String,
    pub tx_message: Option<Vec<u8>>,
    pub tx_signature: Option<String>,
    pub error_message: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Row from the `signing_round_state` table.
#[derive(Debug, Clone, FromRow)]
pub struct SigningRoundStateRow {
    pub id: Uuid,
    pub signing_request_id: Uuid,
    pub node_id: String,
    pub round: i16,
    pub status: String,
    pub output_data: Option<serde_json::Value>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

// ---------------------------------------------------------------------------
// API request types
// ---------------------------------------------------------------------------

/// Request body for `POST /api/signing-requests`.
#[derive(Debug, Deserialize)]
pub struct CreateSigningRequest {
    pub wallet_index: i32,
    pub recipient: String,
    pub amount_lamports: i64,
}

// ---------------------------------------------------------------------------
// API response types
// ---------------------------------------------------------------------------

/// Per-node signing round status map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningNodeRoundStatus {
    pub round1: String,
    pub round2: String,
}

/// Response for `POST /api/signing-requests`.
#[derive(Debug, Serialize)]
pub struct SigningRequestResponse {
    pub id: Uuid,
    pub wallet_index: i32,
    pub sender_address: String,
    pub recipient: String,
    pub amount_lamports: i64,
    pub status: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explorer_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub nodes: std::collections::HashMap<String, SigningNodeRoundStatus>,
}

/// Response for `GET /api/signing-requests`.
#[derive(Debug, Serialize)]
pub struct SigningRequestListResponse {
    pub signing_requests: Vec<SigningRequestResponse>,
}

/// Response for `POST /api/signing-requests/{id}/round/{round}/node/{node_id}`.
#[derive(Debug, Serialize)]
pub struct SigningRoundResponse {
    pub signing_request_id: Uuid,
    pub node_id: String,
    pub round: i16,
    pub status: String,
    pub signing_request_status: String,
    pub nodes: std::collections::HashMap<String, SigningNodeRoundStatus>,
}

/// Response for `POST /api/signing-requests/{id}/aggregate`.
#[derive(Debug, Serialize)]
pub struct AggregateResponse {
    pub signing_request_id: Uuid,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explorer_url: Option<String>,
}
