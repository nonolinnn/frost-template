use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;

// ---------------------------------------------------------------------------
// Database row types
// ---------------------------------------------------------------------------

/// Row from the `wallets` table.
#[derive(Debug, Clone, FromRow)]
pub struct WalletRow {
    pub index: i32,
    pub address: String,
    pub public_key: String,
    pub chain_code: Option<Vec<u8>>,
    pub created_at: OffsetDateTime,
}

// ---------------------------------------------------------------------------
// API response types
// ---------------------------------------------------------------------------

/// Single wallet entry in API responses.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletResponse {
    pub index: i32,
    pub address: String,
    pub public_key: String,
    pub created_at: String,
}

/// Response for `GET /api/wallets`.
#[derive(Debug, Serialize)]
pub struct WalletListResponse {
    pub wallets: Vec<WalletResponse>,
}

/// Response for `GET /api/wallets/{index}/balance`.
#[derive(Debug, Serialize)]
pub struct WalletBalanceResponse {
    pub index: i32,
    pub address: String,
    pub balance_lamports: u64,
    pub balance_sol: f64,
}
