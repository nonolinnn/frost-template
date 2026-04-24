use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Database row types
// ---------------------------------------------------------------------------

/// Row from the `key_shares` table.
#[derive(Debug, Clone, FromRow)]
pub struct KeyShareRow {
    pub id: Uuid,
    pub session_id: Uuid,
    pub key_package: serde_json::Value,
    pub public_key_package: serde_json::Value,
    pub group_public_key: String,
    pub created_at: OffsetDateTime,
}

/// Row from the `dkg_round_data` table.
#[derive(Debug, Clone, FromRow)]
pub struct DkgRoundDataRow {
    pub id: Uuid,
    pub session_id: Uuid,
    pub round: i16,
    pub secret_package: serde_json::Value,
    pub created_at: OffsetDateTime,
}

// ---------------------------------------------------------------------------
// API request types
// ---------------------------------------------------------------------------

/// Request body for `POST /api/dkg/round1`.
#[derive(Debug, Deserialize)]
pub struct DkgRound1Request {
    pub session_id: Uuid,
}

/// Request body for `POST /api/dkg/round2`.
#[derive(Debug, Deserialize)]
pub struct DkgRound2Request {
    pub session_id: Uuid,
    pub round1_packages: std::collections::HashMap<String, serde_json::Value>,
}

/// Request body for `POST /api/dkg/round3`.
#[derive(Debug, Deserialize)]
pub struct DkgRound3Request {
    pub session_id: Uuid,
    pub round1_packages: std::collections::HashMap<String, serde_json::Value>,
    pub round2_packages: std::collections::HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// API response types
// ---------------------------------------------------------------------------

/// Response for `POST /api/dkg/round1`.
#[derive(Debug, Serialize)]
pub struct DkgRound1Response {
    pub node_id: String,
    pub session_id: Uuid,
    pub round1_package: serde_json::Value,
}

/// Response for `POST /api/dkg/round2`.
#[derive(Debug, Serialize)]
pub struct DkgRound2Response {
    pub node_id: String,
    pub session_id: Uuid,
    pub round2_package: serde_json::Value,
}

/// Response for `POST /api/dkg/round3`.
#[derive(Debug, Serialize)]
pub struct DkgRound3Response {
    pub node_id: String,
    pub session_id: Uuid,
    pub group_public_key: String,
    pub verifying_share: String,
}
