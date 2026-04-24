use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::error::{AppError, AppResult};
use crate::models::dkg::{DkgRoundResponse, DkgStartResponse, DkgStatusResponse};
use crate::AppState;

/// Build the DKG sub-router.
///
/// Mounted at `/api/dkg` in the top-level router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/start", post(start_dkg))
        .route("/status", get(get_dkg_status))
        .route("/round/{round}/node/{node_id}", post(execute_round))
}

/// `POST /api/dkg/start` — initialize a new DKG session.
///
/// Stub: returns 501 until fr-004 implements the business logic.
async fn start_dkg(
    State(_state): State<AppState>,
) -> AppResult<Json<DkgStartResponse>> {
    Err(AppError::NotImplemented)
}

/// `GET /api/dkg/status` — get current DKG session status.
///
/// Stub: returns 501 until fr-004 implements the business logic.
async fn get_dkg_status(
    State(_state): State<AppState>,
) -> AppResult<Json<DkgStatusResponse>> {
    Err(AppError::NotImplemented)
}

/// `POST /api/dkg/round/{round}/node/{node_id}` — trigger a DKG round on a node.
///
/// Stub: returns 501 until fr-004 implements the business logic.
async fn execute_round(
    State(_state): State<AppState>,
    Path((round, node_id)): Path<(i16, String)>,
) -> AppResult<Json<DkgRoundResponse>> {
    // Validate inputs even in stub mode so the routing is exercised
    if !(1..=3).contains(&round) {
        return Err(AppError::InvalidRound { round });
    }
    if node_id != "node-a" && node_id != "node-b" {
        return Err(AppError::InvalidNodeId { node_id });
    }
    Err(AppError::NotImplemented)
}
