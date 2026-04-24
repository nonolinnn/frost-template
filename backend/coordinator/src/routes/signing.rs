use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::error::{AppError, AppResult};
use crate::models::signing::{
    AggregateResponse, CreateSigningRequest, SigningRequestListResponse, SigningRequestResponse,
    SigningRoundResponse,
};
use crate::AppState;

/// Build the signing-requests sub-router.
///
/// Mounted at `/api/signing-requests` in the top-level router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_signing_request).get(list_signing_requests))
        .route("/{id}", get(get_signing_request))
        .route("/{id}/round/{round}/node/{node_id}", post(execute_round))
        .route("/{id}/aggregate", post(aggregate))
}

/// `POST /api/signing-requests` -- create a new signing request.
///
/// Stub: returns 501 until fr-006 implements the business logic.
async fn create_signing_request(
    State(_state): State<AppState>,
    Json(_body): Json<CreateSigningRequest>,
) -> AppResult<Json<SigningRequestResponse>> {
    Err(AppError::NotImplemented)
}

/// `GET /api/signing-requests` -- list all signing requests.
///
/// Stub: returns 501 until fr-006 implements the query.
async fn list_signing_requests(
    State(_state): State<AppState>,
) -> AppResult<Json<SigningRequestListResponse>> {
    Err(AppError::NotImplemented)
}

/// `GET /api/signing-requests/{id}` -- get a specific signing request.
///
/// Stub: returns 501 until fr-006 implements the query.
async fn get_signing_request(
    State(_state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> AppResult<Json<SigningRequestResponse>> {
    let _ = id;
    Err(AppError::NotImplemented)
}

/// `POST /api/signing-requests/{id}/round/{round}/node/{node_id}` -- trigger signing round.
///
/// Stub: returns 501 until fr-006 implements the business logic.
async fn execute_round(
    State(_state): State<AppState>,
    Path((id, round, node_id)): Path<(uuid::Uuid, i16, String)>,
) -> AppResult<Json<SigningRoundResponse>> {
    let _ = id;
    // Validate inputs even in stub mode so the routing is exercised
    if !(1..=2).contains(&round) {
        return Err(AppError::InvalidSigningRound { round });
    }
    if node_id != "node-a" && node_id != "node-b" {
        return Err(AppError::InvalidNodeId { node_id });
    }
    Err(AppError::NotImplemented)
}

/// `POST /api/signing-requests/{id}/aggregate` -- aggregate signatures and broadcast.
///
/// Stub: returns 501 until fr-006 implements the business logic.
async fn aggregate(
    State(_state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> AppResult<Json<AggregateResponse>> {
    let _ = id;
    Err(AppError::NotImplemented)
}
