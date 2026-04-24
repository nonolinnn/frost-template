use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};

use crate::error::{AppError, AppResult};
use crate::models::signing::{
    SigningRound1Request, SigningRound1Response, SigningRound2Request, SigningRound2Response,
};
use crate::AppState;

/// Build the signing sub-router for the TSS Node.
///
/// Mounted at `/api/signing` in the top-level router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/round1", post(round1))
        .route("/round2", post(round2))
}

/// `POST /api/signing/round1` -- execute Signing Round 1.
///
/// Stub: returns 501 until fr-006 implements the cryptographic logic.
async fn round1(
    State(_state): State<AppState>,
    Json(_body): Json<SigningRound1Request>,
) -> AppResult<Json<SigningRound1Response>> {
    Err(AppError::NotImplemented)
}

/// `POST /api/signing/round2` -- execute Signing Round 2.
///
/// Stub: returns 501 until fr-006 implements the cryptographic logic.
async fn round2(
    State(_state): State<AppState>,
    Json(_body): Json<SigningRound2Request>,
) -> AppResult<Json<SigningRound2Response>> {
    Err(AppError::NotImplemented)
}
