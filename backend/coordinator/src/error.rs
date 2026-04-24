use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use snafu::Snafu;

/// Application-level error type for the Coordinator service.
///
/// Each variant maps to a specific HTTP status code and a machine-readable
/// `code` string that the frontend can match on.
#[derive(Debug, Snafu)]
pub enum AppError {
    // -- DKG errors ----------------------------------------------------------
    #[snafu(display("A completed DKG session already exists"))]
    DkgAlreadyExists,

    #[snafu(display("A DKG session is already in progress"))]
    DkgInProgress,

    #[snafu(display("No active DKG session found"))]
    DkgSessionNotFound,

    #[snafu(display("Round {round} is not valid (expected 1, 2, or 3)"))]
    InvalidRound { round: i16 },

    #[snafu(display("Node ID '{node_id}' is not valid (expected node-a or node-b)"))]
    InvalidNodeId { node_id: String },

    #[snafu(display("Node {node_id} already completed round {round}"))]
    RoundAlreadyComplete { node_id: String, round: i16 },

    #[snafu(display("Round precondition not met: {message}"))]
    RoundPrecondition { message: String },

    #[snafu(display("Could not reach node {node_id}"))]
    NodeUnavailable { node_id: String },

    #[snafu(display("Node {node_id} returned an error: {message}"))]
    NodeError { node_id: String, message: String },

    // -- Wallet errors -------------------------------------------------------
    #[snafu(display("DKG has not been completed yet"))]
    DkgNotComplete,

    #[snafu(display("No wallet found at index {index}"))]
    WalletNotFound { index: i32 },

    // -- Signing errors ------------------------------------------------------
    #[snafu(display("Invalid recipient address"))]
    InvalidRecipient,

    #[snafu(display("Amount must be greater than zero"))]
    InvalidAmount,

    #[snafu(display("Signing request {id} not found"))]
    SigningRequestNotFound { id: uuid::Uuid },

    #[snafu(display("Invalid signing round {round} (expected 1 or 2)"))]
    InvalidSigningRound { round: i16 },

    #[snafu(display("Signing request is in a terminal state"))]
    InvalidStatus,

    #[snafu(display("Signature aggregation failed: {message}"))]
    AggregationFailed { message: String },

    #[snafu(display("Failed to broadcast transaction: {message}"))]
    BroadcastFailed { message: String },

    #[snafu(display("Solana RPC error: {message}"))]
    SolanaRpcError { message: String },

    // -- Generic errors ------------------------------------------------------
    #[snafu(display("Not implemented"))]
    NotImplemented,

    #[snafu(display("Database error: {message}"))]
    Database { message: String },

    #[snafu(display("Internal server error: {message}"))]
    Internal { message: String },
}

impl AppError {
    /// Machine-readable error code for the JSON response envelope.
    fn error_code(&self) -> &'static str {
        match self {
            Self::DkgAlreadyExists => "DKG_ALREADY_EXISTS",
            Self::DkgInProgress => "DKG_IN_PROGRESS",
            Self::DkgSessionNotFound => "DKG_SESSION_NOT_FOUND",
            Self::InvalidRound { .. } => "INVALID_ROUND",
            Self::InvalidNodeId { .. } => "INVALID_NODE_ID",
            Self::RoundAlreadyComplete { .. } => "ROUND_ALREADY_COMPLETE",
            Self::RoundPrecondition { .. } => "ROUND_PRECONDITION",
            Self::NodeUnavailable { .. } => "NODE_UNAVAILABLE",
            Self::NodeError { .. } => "NODE_ERROR",
            Self::DkgNotComplete => "DKG_NOT_COMPLETE",
            Self::WalletNotFound { .. } => "WALLET_NOT_FOUND",
            Self::InvalidRecipient => "INVALID_RECIPIENT",
            Self::InvalidAmount => "INVALID_AMOUNT",
            Self::SigningRequestNotFound { .. } => "SIGNING_REQUEST_NOT_FOUND",
            Self::InvalidSigningRound { .. } => "INVALID_ROUND",
            Self::InvalidStatus => "INVALID_STATUS",
            Self::AggregationFailed { .. } => "AGGREGATION_FAILED",
            Self::BroadcastFailed { .. } => "BROADCAST_FAILED",
            Self::SolanaRpcError { .. } => "SOLANA_RPC_ERROR",
            Self::NotImplemented => "NOT_IMPLEMENTED",
            Self::Database { .. } => "DATABASE_ERROR",
            Self::Internal { .. } => "INTERNAL_ERROR",
        }
    }

    /// HTTP status code for this error variant.
    fn status_code(&self) -> StatusCode {
        match self {
            Self::DkgAlreadyExists
            | Self::DkgInProgress
            | Self::RoundAlreadyComplete { .. }
            | Self::RoundPrecondition { .. }
            | Self::DkgNotComplete
            | Self::InvalidStatus => StatusCode::CONFLICT,

            Self::InvalidRound { .. }
            | Self::InvalidNodeId { .. }
            | Self::InvalidRecipient
            | Self::InvalidAmount
            | Self::InvalidSigningRound { .. } => StatusCode::BAD_REQUEST,

            Self::DkgSessionNotFound
            | Self::WalletNotFound { .. }
            | Self::SigningRequestNotFound { .. } => StatusCode::NOT_FOUND,

            Self::NodeUnavailable { .. }
            | Self::NodeError { .. }
            | Self::SolanaRpcError { .. }
            | Self::BroadcastFailed { .. } => StatusCode::BAD_GATEWAY,

            Self::AggregationFailed { .. }
            | Self::Database { .. }
            | Self::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,

            Self::NotImplemented => StatusCode::NOT_IMPLEMENTED,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = json!({
            "error": {
                "code": self.error_code(),
                "message": self.to_string(),
            }
        });
        (status, Json(body)).into_response()
    }
}

/// Convenience alias used throughout the coordinator.
pub type AppResult<T> = Result<T, AppError>;

/// Convert sqlx errors into our application error type.
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        tracing::error!("Database error: {err}");
        AppError::Database {
            message: err.to_string(),
        }
    }
}
