use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use snafu::Snafu;

/// Application-level error type for the TSS Node service.
///
/// Each variant maps to a specific HTTP status code and a machine-readable
/// `code` string that the Coordinator can match on.
#[derive(Debug, Snafu)]
pub enum AppError {
    // -- DKG errors ----------------------------------------------------------
    #[snafu(display("This node already completed DKG round {round}"))]
    RoundAlreadyDone { round: i16 },

    #[snafu(display("Round precondition not met: {message}"))]
    RoundPrecondition { message: String },

    #[snafu(display("Invalid or missing packages: {message}"))]
    InvalidPackages { message: String },

    #[snafu(display("Share verification failed: {message}"))]
    VerificationFailed { message: String },

    #[snafu(display("Cryptographic operation failed: {message}"))]
    CryptoError { message: String },

    // -- Signing errors ------------------------------------------------------
    #[snafu(display("DKG has not been completed on this node"))]
    DkgNotComplete,

    #[snafu(display("Invalid commitments: {message}"))]
    InvalidCommitments { message: String },

    #[snafu(display("Invalid message encoding: {message}"))]
    InvalidMessage { message: String },

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
            Self::RoundAlreadyDone { .. } => "ROUND_ALREADY_DONE",
            Self::RoundPrecondition { .. } => "ROUND_PRECONDITION",
            Self::InvalidPackages { .. } => "INVALID_PACKAGES",
            Self::VerificationFailed { .. } => "VERIFICATION_FAILED",
            Self::CryptoError { .. } => "CRYPTO_ERROR",
            Self::DkgNotComplete => "DKG_NOT_COMPLETE",
            Self::InvalidCommitments { .. } => "INVALID_COMMITMENTS",
            Self::InvalidMessage { .. } => "INVALID_MESSAGE",
            Self::NotImplemented => "NOT_IMPLEMENTED",
            Self::Database { .. } => "DATABASE_ERROR",
            Self::Internal { .. } => "INTERNAL_ERROR",
        }
    }

    /// HTTP status code for this error variant.
    fn status_code(&self) -> StatusCode {
        match self {
            Self::RoundAlreadyDone { .. }
            | Self::RoundPrecondition { .. }
            | Self::DkgNotComplete => StatusCode::CONFLICT,

            Self::InvalidPackages { .. }
            | Self::VerificationFailed { .. }
            | Self::InvalidCommitments { .. }
            | Self::InvalidMessage { .. } => StatusCode::BAD_REQUEST,

            Self::CryptoError { .. } | Self::Database { .. } | Self::Internal { .. } => {
                StatusCode::INTERNAL_SERVER_ERROR
            }

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

/// Convenience alias used throughout the tss-node.
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
