use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};

use crate::error::{AppError, AppResult};
use crate::models::dkg::{
    DkgRound1Request, DkgRound1Response, DkgRound2Request, DkgRound2Response, DkgRound3Request,
    DkgRound3Response,
};
use crate::AppState;

/// Build the DKG sub-router for the TSS Node.
///
/// Mounted at `/api/dkg` in the top-level router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/round1", post(round1))
        .route("/round2", post(round2))
        .route("/round3", post(round3))
}

/// `POST /api/dkg/round1` -- execute DKG Round 1.
///
/// Stub: returns 501 until fr-004 implements the cryptographic logic.
async fn round1(
    State(_state): State<AppState>,
    Json(_body): Json<DkgRound1Request>,
) -> AppResult<Json<DkgRound1Response>> {
    Err(AppError::NotImplemented)
}

/// `POST /api/dkg/round2` -- execute DKG Round 2.
///
/// Stub: returns 501 until fr-004 implements the cryptographic logic.
async fn round2(
    State(_state): State<AppState>,
    Json(_body): Json<DkgRound2Request>,
) -> AppResult<Json<DkgRound2Response>> {
    Err(AppError::NotImplemented)
}

/// `POST /api/dkg/round3` -- execute DKG Round 3 (finalize).
///
/// Stub: returns 501 until fr-004 implements the cryptographic logic.
async fn round3(
    State(_state): State<AppState>,
    Json(_body): Json<DkgRound3Request>,
) -> AppResult<Json<DkgRound3Response>> {
    Err(AppError::NotImplemented)
}
