use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use solana_sdk::pubkey::Pubkey;

use crate::db;
use crate::derivation;
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

/// Format a timestamp for API responses.
fn format_ts(ts: time::OffsetDateTime) -> String {
    ts.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| ts.to_string())
}

/// `POST /api/wallets` -- derive the next sequential wallet.
///
/// Uses HD derivation from the group verifying key (produced by DKG).
/// No Node interaction is required.
async fn create_wallet(
    State(state): State<AppState>,
) -> AppResult<(StatusCode, Json<WalletResponse>)> {
    // Precondition: DKG must be complete
    let session = db::dkg::get_completed_session(&state.pool)
        .await?
        .ok_or(AppError::DkgNotComplete)?;

    let group_public_key = session.group_public_key.ok_or_else(|| AppError::Internal {
        message: "DKG complete but group_public_key is null".to_string(),
    })?;

    // Get next sequential index
    let index = db::wallet::next_index(&state.pool).await?;

    // Derive the child public key using hd-wallet Edwards derivation
    let derived = derivation::derive_child_public_key(&group_public_key, index as u32)?;

    // Persist the wallet
    let wallet = db::wallet::insert_wallet(
        &state.pool,
        index,
        &derived.address,
        &derived.public_key,
        Some(&derived.chain_code),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(WalletResponse {
            index: wallet.index,
            address: wallet.address,
            public_key: wallet.public_key,
            created_at: format_ts(wallet.created_at),
        }),
    ))
}

/// `GET /api/wallets` -- list all derived wallets.
async fn list_wallets(State(state): State<AppState>) -> AppResult<Json<WalletListResponse>> {
    let wallets = db::wallet::list_wallets(&state.pool).await?;

    let wallet_responses: Vec<WalletResponse> = wallets
        .into_iter()
        .map(|w| WalletResponse {
            index: w.index,
            address: w.address,
            public_key: w.public_key,
            created_at: format_ts(w.created_at),
        })
        .collect();

    Ok(Json(WalletListResponse {
        wallets: wallet_responses,
    }))
}

/// `GET /api/wallets/{index}/balance` -- query SOL balance for a wallet.
async fn get_balance(
    State(state): State<AppState>,
    Path(index): Path<i32>,
) -> AppResult<Json<WalletBalanceResponse>> {
    // Look up the wallet
    let wallet = db::wallet::get_wallet_by_index(&state.pool, index)
        .await?
        .ok_or(AppError::WalletNotFound { index })?;

    // Decode the Solana address
    let pubkey_bytes = bs58::decode(&wallet.address)
        .into_vec()
        .map_err(|e| AppError::Internal {
            message: format!("Failed to decode wallet address: {e}"),
        })?;

    let pubkey = Pubkey::try_from(pubkey_bytes.as_slice()).map_err(|e| AppError::Internal {
        message: format!("Invalid Solana pubkey: {e}"),
    })?;

    // Query Solana Devnet for balance
    let rpc_client = solana_client::rpc_client::RpcClient::new(&state.config.solana_rpc_url);
    let balance_lamports = rpc_client
        .get_balance(&pubkey)
        .map_err(|e| AppError::SolanaRpcError {
            message: format!("Failed to query balance: {e}"),
        })?;

    let balance_sol = balance_lamports as f64 / 1_000_000_000.0;

    Ok(Json(WalletBalanceResponse {
        index: wallet.index,
        address: wallet.address,
        balance_lamports,
        balance_sol,
    }))
}
