use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::error::{AppError, AppResult};
use crate::models::wallet::{WalletBalanceResponse, WalletListResponse, WalletResponse};
use crate::AppState;

/// Build the wallets sub-router.
///
/// Mounted at `/api/wallets` in the top-level router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_wallet).get(list_wallets))
        .route("/{index}/balance", get(get_balance))
}

/// `POST /api/wallets` — derive the next wallet.
///
/// Stub: returns 501 until fr-005 implements HD derivation.
async fn create_wallet(
    State(_state): State<AppState>,
) -> AppResult<Json<WalletResponse>> {
    Err(AppError::NotImplemented)
}

/// `GET /api/wallets` — list all derived wallets.
///
/// Stub: returns 501 until fr-005 implements the query.
async fn list_wallets(
    State(_state): State<AppState>,
) -> AppResult<Json<WalletListResponse>> {
    Err(AppError::NotImplemented)
}

/// `GET /api/wallets/{index}/balance` — query SOL balance for a wallet.
///
/// Stub: returns 501 until fr-005 implements Solana RPC integration.
async fn get_balance(
    State(_state): State<AppState>,
    Path(index): Path<i32>,
) -> AppResult<Json<WalletBalanceResponse>> {
    // Suppress unused variable warning while maintaining the path extraction
    let _ = index;
    Err(AppError::NotImplemented)
}
