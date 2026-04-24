use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Database row types
// ---------------------------------------------------------------------------

/// Row from the `dkg_sessions` table.
#[derive(Debug, Clone, FromRow)]
pub struct DkgSessionRow {
    pub id: Uuid,
    pub status: String,
    pub group_public_key: Option<String>,
    pub created_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

/// Row from the `dkg_round_state` table.
#[derive(Debug, Clone, FromRow)]
pub struct DkgRoundStateRow {
    pub id: Uuid,
    pub session_id: Uuid,
    pub node_id: String,
    pub round: i16,
    pub status: String,
    pub output_package: Option<serde_json::Value>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

// ---------------------------------------------------------------------------
// API response types
// ---------------------------------------------------------------------------

/// Per-node round status map used in DKG API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRoundStatus {
    pub round1: String,
    pub round2: String,
    pub round3: String,
}

/// Response for `POST /api/dkg/start`.
#[derive(Debug, Serialize)]
pub struct DkgStartResponse {
    pub session_id: Uuid,
    pub status: String,
    pub created_at: String,
    pub nodes: std::collections::HashMap<String, NodeRoundStatus>,
}

/// Response for `GET /api/dkg/status`.
#[derive(Debug, Serialize)]
pub struct DkgStatusResponse {
    pub session_id: Option<Uuid>,
    pub status: String,
    pub created_at: Option<String>,
    pub completed_at: Option<String>,
    pub group_public_key: Option<String>,
    pub nodes: std::collections::HashMap<String, NodeRoundStatus>,
}

/// Response for `POST /api/dkg/round/{round}/node/{node_id}`.
#[derive(Debug, Serialize)]
pub struct DkgRoundResponse {
    pub session_id: Uuid,
    pub node_id: String,
    pub round: i16,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dkg_complete: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_public_key: Option<String>,
    pub nodes: std::collections::HashMap<String, NodeRoundStatus>,
}
